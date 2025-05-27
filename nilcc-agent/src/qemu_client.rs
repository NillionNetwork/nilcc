use qapi::{
    futures::{QapiService, QapiStream, QmpStreamNegotiation, QmpStreamTokio},
    qmp::{query_cpus_fast, quit, system_powerdown, system_reset, QmpCommand},
    Command as QapiCommandTrait, ExecuteError,
};
use serde::{Deserialize, Serialize};
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    process::ExitStatus,
};
use thiserror::Error;
use tokio::{
    fs,
    io::{ReadHalf, WriteHalf},
    net::UnixStream,
    process::Command,
    task::JoinHandle,
};
use tracing::debug;

type QmpReadStreamHalf = QmpStreamTokio<ReadHalf<UnixStream>>;
type QmpWriteStreamHalf = QmpStreamTokio<WriteHalf<UnixStream>>;
type NegotiatedQmpStream = QapiStream<QmpReadStreamHalf, QmpWriteStreamHalf>;
type QmpCommandService = QapiService<QmpWriteStreamHalf>;
type QmpDriverTaskHandle = JoinHandle<()>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmSpec {
    /// Number of vCPUs to allocate to the VM.
    pub cpu: u8,

    /// Amount of RAM to allocate to the VM (in MiB).
    pub ram_mib: u32,

    /// Size of the disk image to create (in GiB).
    pub disk_gib: u32,

    /// Optional ISO path to attach as CD-ROM.
    #[serde(default)]
    pub cdrom_iso_path: Option<PathBuf>,

    /// If true, add a VFIO GPU passthrough device (`-device vfio-pci,…`).
    #[serde(default)]
    pub gpu_enabled: bool,

    /// Vec of (HOST, GUEST) ports to forward.
    #[serde(default)]
    pub port_forwarding: Vec<(u16, u16)>,

    /// Optional Bios path to use for the VM.
    #[serde(default)]
    pub bios_path: Option<PathBuf>,

    /// If true, show VM display using GTK.
    #[serde(default)]
    pub display_gtk: bool,

    /// Enable CVM (Confidential VM) support.
    #[serde(default)]
    pub enable_cvm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmDetails {
    pub name: String,
    pub qmp_sock: PathBuf,
    pub pid: Option<u32>,
    pub workdir: PathBuf,
    pub spec: VmSpec,
}

#[derive(Error, Debug)]
pub enum QemuClientError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("QMP Error: {0}")]
    Qmp(String),

    #[error("VM not found")]
    VmNotFound,

    #[error("VM already exists")]
    VmAlreadyExists,

    #[error("VM already running")]
    VmAlreadyRunning,

    #[error("VM not running")]
    VmNotRunning,

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("cannot find GPU: {0}")]
    Gpu(String),
}

pub type Result<T> = std::result::Result<T, QemuClientError>;

pub struct QemuClient {
    qemu_bin: PathBuf,
    qemu_img_bin: PathBuf,
    store: PathBuf,
}

pub struct CommandOutput {
    pub status: ExitStatus,
    pub stderr: String,
}

impl QemuClient {
    pub fn new<P: Into<PathBuf>>(qemu_bin: P, qemu_img_bin: P, store: P) -> Self {
        Self { qemu_bin: qemu_bin.into(), qemu_img_bin: qemu_img_bin.into(), store: store.into() }
    }

    /// Build complete QEMU command-line
    async fn build_command_line(&self, workdir: &Path, spec: &VmSpec) -> Result<(PathBuf, Vec<String>)> {
        let disk = workdir.join("disk.qcow2");
        let qmp_sock = workdir.join("qmp.sock");

        let mut args: Vec<String> = Vec::new();

        // --- CVM support ---
        if spec.enable_cvm {
            args.extend([
                "-machine".into(),
                "confidential-guest-support=sev0,vmport=off".into(),
                "-object".into(),
                "sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1".into(),
            ]);
        }

        // --- Base machine + CPU / RAM ---
        args.extend([
            "-enable-kvm".into(),
            "-no-reboot".into(),
            "-cpu".into(),
            "EPYC-v4".into(),
            "-smp".into(),
            spec.cpu.to_string(),
            "-m".into(),
            spec.ram_mib.to_string(),
            "-machine".into(),
            "q35,accel=kvm".into(),
        ]);

        // --- Main system drive ---
        args.extend([
            "-drive".into(),
            format!("file={},if=none,id=disk0,format=qcow2", disk.display()),
            "-device".into(),
            "virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true".into(),
            "-device".into(),
            "scsi-hd,drive=disk0".into(),
        ]);

        // --- Network backend ---
        args.extend(["-device".into(), "pcie-root-port,id=pci.1,bus=pcie.0".into()]);

        args.extend([
            "-fw_cfg".into(),
            "name=opt/ovmf/X-PciMmio64Mb,string=151072".into(),
            "-qmp".into(),
            format!("unix:{},server,nowait", qmp_sock.display()),
            "-daemonize".into(),
        ]);

        // --- CD-ROM ---
        if let Some(iso) = &spec.cdrom_iso_path {
            args.push("-drive".into());
            args.push(format!("file={},media=cdrom,readonly=on", iso.display()));
        }

        // --- BIOS ---
        if let Some(bios) = &spec.bios_path {
            args.extend(["-bios".into(), bios.display().to_string()]);
        }

        // --- Display ---
        if spec.display_gtk {
            args.extend(["-device".into(), "virtio-vga".into(), "-display".into(), "gtk,gl=off".into()]);
        } else {
            args.extend(["-display".into(), "none".into()]);
        }

        // --- Network and Port forwarding ---
        args.push("-netdev".into());
        let fwd = spec
            .port_forwarding
            .iter()
            .map(|(h, g)| format!("hostfwd=tcp::{}-:{}", h, g))
            .collect::<Vec<_>>()
            .join(",");

        if fwd.is_empty() {
            args.push("user,id=vmnic".into());
        } else {
            args.push(format!("user,id=vmnic,{}", fwd));
        }
        args.push("-device".into());
        args.push("virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile=".into());

        // --- GPU passthrough ---
        if spec.gpu_enabled {
            let gpu = Self::find_gpu().await?;
            args.extend([
                "-device".into(),
                "pcie-root-port,id=pci.1,bus=pcie.0".into(),
                "-device".into(),
                format!("vfio-pci,host={},bus=pci.1", gpu),
            ]);
        }

        Ok((disk, args))
    }

    async fn find_gpu() -> Result<String> {
        // Resolve first NVIDIA PCI BDF
        let result =
            Command::new("bash").arg("-c").arg("lspci -d 10de: | awk '/NVIDIA/{print $1}' | head -n1").output().await;
        let output = match result {
            Ok(output) => output,
            Err(e) => return Err(QemuClientError::Gpu(format!("failed to invoke command: {e}"))),
        };
        if !output.status.success() {
            return Err(QemuClientError::Gpu(format!(
                "executing command failed with status code: {}",
                output.status.code().unwrap_or_default()
            )));
        }
        let bdf = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if bdf.is_empty() {
            Err(QemuClientError::Gpu("no GPU found".into()))
        } else {
            Ok(bdf)
        }
    }

    async fn load_details(&self, name: &str) -> Result<VmDetails> {
        let json = self.store.join(name).join("vm.json");
        if !fs::try_exists(&json).await? {
            return Err(QemuClientError::VmNotFound);
        }
        Ok(serde_json::from_str(&fs::read_to_string(json).await?)?)
    }

    async fn is_running(&self, qmp_sock: &Path) -> bool {
        match QmpStreamTokio::open_uds(qmp_sock).await {
            Ok(stream) => stream.negotiate().await.is_ok(),
            Err(_) => false,
        }
    }

    async fn connect_qmp(&self, qmp_sock_path: &Path) -> Result<(QmpCommandService, QmpDriverTaskHandle)> {
        debug!("Connecting to QMP socket at: {}", qmp_sock_path.display());

        let pre_negotiation_stream: QmpStreamNegotiation<QmpReadStreamHalf, QmpWriteStreamHalf> =
            QmpStreamTokio::open_uds(qmp_sock_path).await.map_err(|io_err| {
                QemuClientError::Qmp(format!(
                    "Failed to connect to QMP socket at '{}': {}",
                    qmp_sock_path.display(),
                    io_err
                ))
            })?;

        let negotiated_stream: NegotiatedQmpStream =
            pre_negotiation_stream.negotiate().await.map_err(|io_err: std::io::Error| {
                QemuClientError::Qmp(format!(
                    "QMP negotiation failed for socket at '{}': {}",
                    qmp_sock_path.display(),
                    io_err
                ))
            })?;

        debug!("QMP connection established and negotiated for socket: {}", qmp_sock_path.display());
        Ok(negotiated_stream.spawn_tokio())
    }

    pub async fn execute_qmp_command<C>(&self, qmp_sock: &Path, command: C) -> Result<<C as QapiCommandTrait>::Ok>
    where
        C: QapiCommandTrait + QmpCommand,
    {
        if !self.is_running(qmp_sock).await {
            return Err(QemuClientError::VmNotRunning);
        }

        let (qmp, driver) = self.connect_qmp(qmp_sock).await?;

        let response = qmp.execute(command).await.map_err(|exec_err: ExecuteError| {
            QemuClientError::Qmp(format!("QMP command '{}' execution failed:: {}", C::NAME, exec_err))
        })?;

        // Explicitly drop the service handle to signal that no more commands will be sent and to allow the driver_task to shut down if it depends on this.
        drop(qmp);
        driver.await.map_err(|e| QemuClientError::Qmp(e.to_string()))?;

        Ok(response)
    }

    /// Create and start a brand-new VM. Fails if the VM directory already exists.
    pub async fn create_vm(&self, name: &str, spec: VmSpec) -> Result<VmDetails> {
        let workdir = self.store.join(name);
        let meta_json = workdir.join("vm.json");

        if fs::try_exists(&meta_json).await? {
            return Err(QemuClientError::VmAlreadyExists);
        }
        fs::create_dir_all(&workdir).await?;

        let (disk, _) = self.build_command_line(&workdir, &spec).await?;
        let args = ["create", "-f", "qcow2", disk.to_str().unwrap(), &format!("{}G", spec.disk_gib)];
        let output = Self::invoke_cli_command(&self.qemu_img_bin, &args).await?;
        if !output.status.success() {
            return Err(QemuClientError::Io(std::io::Error::other(format!("qemu-img failed: {}", output.stderr))));
        }

        let details = VmDetails { name: name.into(), qmp_sock: workdir.join("qmp.sock"), pid: None, workdir, spec };
        fs::write(&meta_json, serde_json::to_string_pretty(&details)?).await?;

        self.start_vm(name).await
    }

    /// Start an existing (stopped) VM.
    pub async fn start_vm(&self, name: &str) -> Result<VmDetails> {
        let details = self.load_details(name).await?;

        if self.is_running(&details.qmp_sock).await {
            return Err(QemuClientError::VmAlreadyRunning);
        }

        let (_, args) = self.build_command_line(&details.workdir, &details.spec).await?;
        let args: Vec<_> = args.iter().map(Deref::deref).collect();

        let output = Self::invoke_cli_command(&self.qemu_bin, &args).await?;
        if !output.status.success() {
            return Err(QemuClientError::Io(std::io::Error::other(format!("qemu failed: {}", output.stderr))));
        }

        while !fs::try_exists(&details.qmp_sock).await? {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Ok(details)
    }

    /// Verify running VM matches spec.
    pub async fn check_vm_spec(&self, name: &str) -> Result<VmDetails> {
        let details = self.load_details(name).await?;

        // TODO: add more checks here, like checking disk size, RAM, etc.
        let cpus = self.execute_qmp_command(&details.qmp_sock, query_cpus_fast {}).await?;

        if cpus.len() as u8 != details.spec.cpu {
            return Err(QemuClientError::Qmp(format!(
                "CPU mismatch: running={}, expected={}",
                cpus.len(),
                details.spec.cpu
            )));
        }
        Ok(details)
    }

    /// Restart the VM
    pub async fn restart_vm(&self, name: &str) -> Result<VmDetails> {
        let details = self.load_details(name).await?;
        self.execute_qmp_command(&details.qmp_sock, system_reset {}).await?;
        Ok(details)
    }

    /// Gracefully stop the VM, waiting for it to shut down
    pub async fn stop_vm(&self, name: &str, force: bool) -> Result<VmDetails> {
        let details = self.load_details(name).await?;

        if force {
            self.execute_qmp_command(&details.qmp_sock, quit {}).await?;
        } else {
            self.execute_qmp_command(&details.qmp_sock, system_powerdown {}).await?;
        };

        Ok(details)
    }

    /// Delete VM directory after best effort kill
    pub async fn delete_vm(&self, name: &str) -> Result<VmDetails> {
        let details = self.load_details(name).await?;

        // Kill vm if it's running
        let _ = Command::new("pkill").args(["-f", &details.qmp_sock.to_string_lossy()]).output().await;

        fs::remove_dir_all(&details.workdir).await?;
        Ok(details)
    }

    /// Get VM status
    pub async fn vm_status(&self, name: &str) -> Result<(VmDetails, bool)> {
        let details = self.load_details(name).await?;
        let running = self.is_running(&details.qmp_sock).await;
        Ok((details, running))
    }

    async fn invoke_cli_command(command: &Path, args: &[&str]) -> Result<CommandOutput> {
        debug!("Executing: {} {}", command.display(), args.join(" "));

        let output = Command::new(command).args(args).output().await?;

        Ok(CommandOutput { status: output.status, stderr: String::from_utf8_lossy(&output.stderr).trim().to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tracing_test::traced_test;

    fn make_client(workdir: &Path) -> QemuClient {
        QemuClient::new(Path::new("qemu-system-x86_64"), Path::new("qemu-img"), workdir)
    }

    #[test_with::no_env(GITHUB_ACTIONS)]
    #[tokio::test]
    #[traced_test]
    async fn build_cmd_contains_resources() {
        let workdir = Path::new("/tmp").join("dummy");
        let client = make_client(&workdir);
        let spec = VmSpec {
            cpu: 2,
            ram_mib: 2048,
            disk_gib: 20,
            cdrom_iso_path: None,
            gpu_enabled: false,
            port_forwarding: vec![],
            bios_path: None,
            display_gtk: false,
            enable_cvm: false,
        };
        let (disk, args) = client.build_command_line(&workdir, &spec).await.expect("failed to build command line");
        assert_eq!(disk, workdir.join("disk.qcow2"));
        assert!(args.contains(&"2048".to_owned()));
        assert!(args.contains(&"2".to_owned()));
        assert!(args.iter().any(|a| a.contains("qmp.sock")));
    }

    #[test_with::no_env(GITHUB_ACTIONS)]
    #[tokio::test]
    #[traced_test]
    async fn vm_lifecycle() {
        let store = PathBuf::from("/tmp/nilcc-test-vms");
        let client = make_client(&store);
        for bin in [&client.qemu_bin, &client.qemu_img_bin] {
            if Command::new(bin).arg("--version").output().await.is_err() {
                eprintln!("QEMU binary {} not found or not executable", bin.display());
                return;
            }
        }

        let vm_name = "test_vm";

        let _ = fs::remove_dir_all(&store).await;
        fs::create_dir_all(&store).await.unwrap();

        let spec = VmSpec {
            cpu: 1,
            ram_mib: 512,
            disk_gib: 1,
            cdrom_iso_path: None,
            gpu_enabled: false,
            port_forwarding: vec![],
            bios_path: None,
            display_gtk: false,
            enable_cvm: false,
        };

        let details = client.create_vm(vm_name, spec).await.unwrap();
        assert!(client.is_running(&details.qmp_sock).await);

        client.check_vm_spec(vm_name).await.unwrap();
        client.stop_vm(vm_name, false).await.unwrap();
        client.delete_vm(vm_name).await.unwrap();

        assert!(!store.join(vm_name).exists());
        let _ = fs::remove_dir_all(&store).await;
    }
}
