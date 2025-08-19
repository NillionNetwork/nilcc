use crate::resources::GpuAddress;
use async_trait::async_trait;
use qapi::{
    futures::{QapiService, QapiStream, QmpStreamNegotiation, QmpStreamTokio},
    qmp::{quit, system_powerdown, system_reset, QmpCommand},
    Command as QapiCommandTrait, ExecuteError,
};
use std::{
    fmt, io,
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

/// The spec for a hard disk.
#[derive(Debug, Clone, PartialEq)]
pub struct HardDiskSpec {
    /// The path to the hard disk.
    pub path: PathBuf,

    /// The hard disk format
    pub format: HardDiskFormat,

    /// Whether the disk should be set to read only.
    pub read_only: bool,
}

/// A hard disk format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HardDiskFormat {
    /// A hard disk in raw format.
    Raw,

    /// A hard disk in qcow2 format.
    Qcow2,
}

impl fmt::Display for HardDiskFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format = match self {
            Self::Raw => "raw",
            Self::Qcow2 => "qcow2",
        };
        write!(f, "{format}")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VmSpec {
    /// Number of vCPUs to allocate to the VM.
    pub cpu: u32,

    /// Amount of RAM to allocate to the VM (in MiB).
    pub ram_mib: u32,

    /// The disks to attach.
    pub hard_disks: Vec<HardDiskSpec>,

    /// Optional ISO path to attach as CD-ROM.
    pub cdrom_iso_path: Option<PathBuf>,

    /// The GPU addresses to use.
    pub gpus: Vec<GpuAddress>,

    /// Vec of (HOST, GUEST) ports to forward.
    pub port_forwarding: Vec<(u16, u16)>,

    /// Optional BIOS path to use for the VM.
    pub bios_path: Option<PathBuf>,

    /// Optional kernel path to use for the VM.
    pub initrd_path: Option<PathBuf>,

    /// Optional kernel path to use for the VM.
    pub kernel_path: Option<PathBuf>,

    /// The kernel parameters.
    pub kernel_args: Option<String>,

    /// If true, show VM display using GTK.
    pub display_gtk: bool,

    /// Enable CVM (Confidential VM) support.
    pub enable_cvm: bool,
}

#[derive(Error, Debug)]
pub enum QemuClientError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),

    #[error("QMP Error: {0}")]
    Qmp(String),

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

/// A client to manage VMs
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmClient: Send + Sync {
    /// Start a VM with the given spec that we will be able to talk to via the socket in the given
    /// path.
    async fn start_vm(&self, socket_path: &Path, spec: VmSpec) -> Result<()>;

    /// Restart a VM.
    async fn restart_vm(&self, socket_path: &Path) -> Result<()>;

    /// Stop a VM.
    async fn stop_vm(&self, socket_path: &Path, force: bool) -> Result<()>;

    /// Check if a VM is running.
    async fn is_vm_running(&self, socket_path: &Path) -> bool;
}

pub struct QemuClient {
    qemu_bin: PathBuf,
}

pub struct CommandOutput {
    pub status: ExitStatus,
    pub stderr: String,
}

impl QemuClient {
    pub fn new<P: Into<PathBuf>>(qemu_bin: P) -> Self {
        Self { qemu_bin: qemu_bin.into() }
    }

    /// Build complete QEMU command-line args for starting a VM.
    async fn build_vm_start_cli_args(&self, spec: &VmSpec, socket_path: &Path) -> Result<Vec<String>> {
        let mut args: Vec<String> = Vec::new();

        // --- CVM support ---
        if spec.enable_cvm {
            args.extend([
                "-machine".into(),
                "confidential-guest-support=sev0,vmport=off".into(),
                "-object".into(),
                "sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,kernel-hashes=on".into(),
            ]);
        }

        // --- Display ---
        if spec.display_gtk {
            args.extend(["-device".into(), "virtio-vga".into(), "-display".into(), "gtk,gl=off".into()]);
        } else {
            args.extend(["-display".into(), "none".into()]);
        }

        // --- Base machine + CPU / RAM ---
        args.extend([
            "-enable-kvm".into(),
            "-no-reboot".into(),
            "-daemonize".into(),
            "-cpu".into(),
            "EPYC-v4".into(),
            "-smp".into(),
            spec.cpu.to_string(),
            "-m".into(),
            spec.ram_mib.to_string(),
            "-machine".into(),
            "q35,accel=kvm".into(),
            "-fw_cfg".into(),
            "name=opt/ovmf/X-PciMmio64Mb,string=151072".into(),
            "-qmp".into(),
            format!("unix:{},server,nowait", socket_path.display()),
        ]);

        // --- BIOS ---
        if let Some(bios) = &spec.bios_path {
            args.extend(["-bios".into(), bios.display().to_string()]);
        }

        // --- initrd ---
        if let Some(initrd) = &spec.initrd_path {
            args.extend(["-initrd".into(), initrd.display().to_string()]);
        }

        // --- Kernel ---
        if let Some(kernel) = &spec.kernel_path {
            args.extend(["-kernel".into(), kernel.display().to_string()]);
        }

        // --- kernel command line --
        if let Some(cmdline) = &spec.kernel_args {
            args.extend(["-append".into(), cmdline.clone()]);
        }

        // --- Main system drive ---
        let mut scsi_device_count = 0;
        for disk in &spec.hard_disks {
            let HardDiskSpec { path, format, read_only } = disk;
            let path_display = path.display();
            let disk_id = format!("disk{scsi_device_count}");
            let scsi_id = format!("scsi{scsi_device_count}");
            let read_only_opt = if *read_only { ",read-only=on" } else { "" };
            args.extend([
                "-drive".into(),
                format!("file={path_display},if=none,id={disk_id},format={format}{read_only_opt}"),
                "-device".into(),
                format!("virtio-scsi-pci,id={scsi_id},disable-legacy=on,iommu_platform=true"),
                "-device".into(),
                format!("scsi-hd,drive={disk_id}"),
            ]);
            scsi_device_count += 1;
        }

        // --- CD-ROM ---
        if let Some(iso) = &spec.cdrom_iso_path {
            let path_display = iso.display();
            let disk_id = format!("disk{scsi_device_count}");
            let scsi_id = format!("scsi{scsi_device_count}");
            args.extend([
                "-drive".into(),
                format!("file={path_display},if=none,id={disk_id},readonly=true"),
                "-device".into(),
                format!("virtio-scsi-pci,id={scsi_id}"),
                "-device".into(),
                format!("scsi-cd,bus={scsi_id}.0,drive={disk_id}"),
            ]);
        }

        // --- Network and Port forwarding ---
        if !spec.port_forwarding.is_empty() {
            let fwd = spec
                .port_forwarding
                .iter()
                .map(|(h, g)| format!("hostfwd=tcp:127.0.0.1:{h}-:{g}"))
                .collect::<Vec<_>>()
                .join(",");
            args.extend([
                "-device".into(),
                "virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile=".into(),
                "-netdev".into(),
                format!("user,id=vmnic,{fwd}"),
            ]);
        }

        // --- GPU passthrough ---
        for (index, gpu) in spec.gpus.iter().enumerate() {
            let gpu = &gpu.0;
            let id = format!("gpu{}", index + 1);
            args.extend([
                "-device".into(),
                format!("pcie-root-port,id={id},bus=pcie.0"),
                "-device".into(),
                format!("vfio-pci,host={gpu},bus={id}"),
            ]);
        }

        Ok(args)
    }

    async fn connect_qmp(&self, qmp_sock_path: &Path) -> Result<(QmpCommandService, QmpDriverTaskHandle)> {
        debug!("Connecting to QMP socket at: {}", qmp_sock_path.display());

        let pre_negotiation_stream: QmpStreamNegotiation<QmpReadStreamHalf, QmpWriteStreamHalf> =
            QmpStreamTokio::open_uds(qmp_sock_path).await.map_err(|io_err| {
                QemuClientError::Qmp(format!(
                    "Failed to connect to QMP socket at '{}': {io_err}",
                    qmp_sock_path.display()
                ))
            })?;

        let negotiated_stream: NegotiatedQmpStream =
            pre_negotiation_stream.negotiate().await.map_err(|io_err: io::Error| {
                QemuClientError::Qmp(format!(
                    "QMP negotiation failed for socket at '{}': {io_err}",
                    qmp_sock_path.display()
                ))
            })?;

        debug!("QMP connection established and negotiated for socket: {}", qmp_sock_path.display());
        Ok(negotiated_stream.spawn_tokio())
    }

    async fn execute_qmp_command<C>(&self, qmp_sock: &Path, command: C) -> Result<<C as QapiCommandTrait>::Ok>
    where
        C: QapiCommandTrait + QmpCommand,
    {
        if !self.is_vm_running(qmp_sock).await {
            return Err(QemuClientError::VmNotRunning);
        }

        let (qmp, driver) = self.connect_qmp(qmp_sock).await?;

        let response = qmp.execute(command).await.map_err(|exec_err: ExecuteError| {
            QemuClientError::Qmp(format!("QMP command '{}' execution failed:: {exec_err}", C::NAME))
        })?;

        // Explicitly drop the service handle to signal that no more commands will be sent and to allow the driver_task to shut down if it depends on this.
        drop(qmp);
        driver.await.map_err(|e| QemuClientError::Qmp(e.to_string()))?;

        Ok(response)
    }

    async fn invoke_cli_command(command: &Path, args: &[&str]) -> Result<CommandOutput> {
        debug!("Executing: {} {}", command.display(), args.join(" "));

        let output = Command::new(command).args(args).output().await?;

        Ok(CommandOutput { status: output.status, stderr: String::from_utf8_lossy(&output.stderr).trim().to_string() })
    }
}

#[async_trait]
impl VmClient for QemuClient {
    /// Start an existing (stopped) VM.
    async fn start_vm(&self, socket_path: &Path, spec: VmSpec) -> Result<()> {
        if self.is_vm_running(socket_path).await {
            return Err(QemuClientError::VmAlreadyRunning);
        }

        let args = self.build_vm_start_cli_args(&spec, socket_path).await?;
        let args: Vec<_> = args.iter().map(Deref::deref).collect();

        let output = Self::invoke_cli_command(&self.qemu_bin, &args).await?;
        if !output.status.success() {
            return Err(QemuClientError::Io(io::Error::other(format!("qemu failed: {}", output.stderr))));
        }

        while !fs::try_exists(socket_path).await? {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }

    /// Restart the VM
    async fn restart_vm(&self, socket_path: &Path) -> Result<()> {
        self.execute_qmp_command(socket_path, system_reset {}).await?;
        Ok(())
    }

    /// Gracefully stop the VM, waiting for it to shut down
    async fn stop_vm(&self, socket_path: &Path, force: bool) -> Result<()> {
        if force {
            self.execute_qmp_command(socket_path, quit {}).await?;
        } else {
            self.execute_qmp_command(socket_path, system_powerdown {}).await?;
        }
        Ok(())
    }

    async fn is_vm_running(&self, socket_path: &Path) -> bool {
        match QmpStreamTokio::open_uds(socket_path).await {
            Ok(stream) => stream.negotiate().await.is_ok(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::disk::{DefaultDiskService, DiskService};
    use tracing_test::traced_test;

    fn make_client() -> QemuClient {
        QemuClient::new(Path::new("qemu-system-x86_64"))
    }

    #[tokio::test]
    #[traced_test]
    async fn build_cmd_contains_resources() {
        let client = make_client();
        let spec = VmSpec {
            cpu: 2,
            ram_mib: 2048,
            hard_disks: vec![
                HardDiskSpec { path: "/tmp/1.qcow2".into(), format: HardDiskFormat::Qcow2, read_only: true },
                HardDiskSpec { path: "/tmp/2.raw".into(), format: HardDiskFormat::Raw, read_only: false },
            ],
            cdrom_iso_path: Some("/tmp/cd.iso".into()),
            gpus: vec![GpuAddress("A".into()), GpuAddress("B".into())],
            port_forwarding: vec![(8080, 80)],
            bios_path: Some("/tmp/bios".into()),
            initrd_path: Some("/tmp/initrd".into()),
            kernel_path: Some("/tmp/kernel".into()),
            kernel_args: Some("root=/dev/foo1".into()),
            display_gtk: false,
            enable_cvm: true,
        };
        let socket_path = Path::new("/tmp/vm.socket");
        let args = client.build_vm_start_cli_args(&spec, &socket_path).await.expect("failed to build command line");
        let expected = [
            // SNP
            "-machine",
            "confidential-guest-support=sev0,vmport=off",
            "-object",
            "sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,kernel-hashes=on",
            // Display
            "-display",
            "none",
            // Basic configs
            "-enable-kvm",
            "-no-reboot",
            "-daemonize",
            // CPU
            "-cpu",
            "EPYC-v4",
            "-smp",
            "2",
            "-m",
            "2048",
            "-machine",
            "q35,accel=kvm",
            // Firmware
            "-fw_cfg",
            "name=opt/ovmf/X-PciMmio64Mb,string=151072",
            // QMP socket
            "-qmp",
            "unix:/tmp/vm.socket,server,nowait",
            // BIOS
            "-bios",
            "/tmp/bios",
            // initrd
            "-initrd",
            "/tmp/initrd",
            // kernel
            "-kernel",
            "/tmp/kernel",
            // kernel cmdline
            "-append",
            "root=/dev/foo1",
            // Drive 1 (.qcow2)
            "-drive",
            "file=/tmp/1.qcow2,if=none,id=disk0,format=qcow2,read-only=on",
            "-device",
            "virtio-scsi-pci,id=scsi0,disable-legacy=on,iommu_platform=true",
            "-device",
            "scsi-hd,drive=disk0",
            // Drive 2 (.raw)
            "-drive",
            "file=/tmp/2.raw,if=none,id=disk1,format=raw",
            "-device",
            "virtio-scsi-pci,id=scsi1,disable-legacy=on,iommu_platform=true",
            "-device",
            "scsi-hd,drive=disk1",
            // cdrom
            "-drive",
            "file=/tmp/cd.iso,if=none,id=disk2,readonly=true",
            "-device",
            "virtio-scsi-pci,id=scsi2",
            "-device",
            "scsi-cd,bus=scsi2.0,drive=disk2",
            // Network
            "-device",
            "virtio-net-pci,disable-legacy=on,iommu_platform=true,netdev=vmnic,romfile=",
            // Port forward
            "-netdev",
            "user,id=vmnic,hostfwd=tcp:127.0.0.1:8080-:80",
            // GPUs
            "-device",
            "pcie-root-port,id=gpu1,bus=pcie.0",
            "-device",
            "vfio-pci,host=A,bus=gpu1",
            "-device",
            "pcie-root-port,id=gpu2,bus=pcie.0",
            "-device",
            "vfio-pci,host=B,bus=gpu2",
        ];
        assert_eq!(args, expected);
    }

    #[test_with::no_env(GITHUB_ACTIONS)]
    #[tokio::test]
    #[traced_test]
    async fn vm_lifecycle() {
        let store = tempfile::tempdir().expect("failed to create tempdir");
        let client = make_client();

        let _ = fs::remove_dir_all(&store).await;
        fs::create_dir_all(&store).await.unwrap();

        let hard_disk_path = store.path().join("disk.qcow2");
        let hard_disk_format = HardDiskFormat::Qcow2;
        DefaultDiskService::new("qemu-img".into())
            .create_disk(&hard_disk_path, hard_disk_format, 1)
            .await
            .expect("failed to create hard disk");

        let spec = VmSpec {
            cpu: 1,
            ram_mib: 512,
            hard_disks: vec![HardDiskSpec { path: hard_disk_path, format: hard_disk_format, read_only: true }],
            cdrom_iso_path: None,
            gpus: Vec::new(),
            port_forwarding: vec![],
            bios_path: None,
            initrd_path: None,
            kernel_path: None,
            kernel_args: None,
            display_gtk: false,
            enable_cvm: false,
        };

        let socket_path = store.path().join("vm.sock");

        // Start it and make sure it's running
        client.start_vm(&socket_path, spec).await.unwrap();
        assert!(client.is_vm_running(&socket_path).await);

        // Stop is and make sure it's not running anymore.
        client.stop_vm(&socket_path, true).await.unwrap();
        assert!(!client.is_vm_running(&socket_path).await);
    }
}
