use crate::{
    clients::{
        cvm_agent::CvmAgentClient,
        qemu::{HardDiskFormat, HardDiskSpec, VmClient, VmSpec},
    },
    config::{CvmFiles, DockerConfig, ZeroSslConfig},
    repositories::workload::Workload,
    services::disk::{ApplicationMetadata, ContainerMetadata, DiskService, EnvironmentVariable, ExternalFile, IsoSpec},
    workers::{
        events::EventSender,
        vm::{VmWorker, VmWorkerArgs, VmWorkerHandle},
    },
};
use anyhow::Context;
use async_trait::async_trait;
use cvm_agent_models::bootstrap::DockerCredentials;
use nilcc_artifacts::VmType;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::Mutex};
use tracing::{error, info};
use uuid::Uuid;

const CVM_AGENT_PORT: u16 = 59666;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmService: Send + Sync {
    async fn create_vm(&self, workload: Workload) -> Result<(), StartVmError>;
    async fn create_workload_spec(&self, workload: &Workload) -> Result<VmSpec, StartVmError>;
    async fn delete_vm(&self, id: Uuid);
    async fn restart_vm(&self, id: Uuid) -> Result<(), VmNotManaged>;
}

#[derive(Debug, thiserror::Error)]
#[error("vm is not managed by any worker")]
pub struct VmNotManaged;

pub struct VmServiceArgs {
    pub state_path: PathBuf,
    pub vm_client: Arc<dyn VmClient>,
    pub cvm_agent_client: Arc<dyn CvmAgentClient>,
    pub disk_service: Box<dyn DiskService>,
    pub base_cvm_config_path: PathBuf,
    pub zerossl_config: ZeroSslConfig,
    pub docker_config: DockerConfig,
    pub event_sender: EventSender,
}

pub struct DefaultVmService {
    vm_client: Arc<dyn VmClient>,
    cvm_agent_client: Arc<dyn CvmAgentClient>,
    disk_service: Box<dyn DiskService>,
    workers: Mutex<HashMap<Uuid, VmWorkerHandle>>,
    state_path: PathBuf,
    base_cvm_config_path: PathBuf,
    zerossl_config: ZeroSslConfig,
    docker_config: DockerConfig,
    event_sender: EventSender,
}

impl DefaultVmService {
    pub async fn new(args: VmServiceArgs) -> anyhow::Result<Self> {
        let VmServiceArgs {
            state_path,
            vm_client,
            cvm_agent_client,
            disk_service,
            base_cvm_config_path,
            zerossl_config,
            docker_config,
            event_sender,
        } = args;
        fs::create_dir_all(&state_path).await.context("Creating state directory")?;
        Ok(Self {
            vm_client,
            cvm_agent_client,
            disk_service,
            workers: Default::default(),
            state_path,
            base_cvm_config_path,
            zerossl_config,
            docker_config,
            event_sender,
        })
    }

    fn create_vm_spec(
        &self,
        workload: &Workload,
        iso_path: PathBuf,
        state_disk_path: PathBuf,
        docker_compose_hash: String,
        cvm_config: CvmConfig,
    ) -> VmSpec {
        let CvmFiles { kernel, base_disk, verity_root_hash, verity_disk, .. } = cvm_config.vm;
        let kernel_args =
            KernelArgs { filesystem_root_hash: &verity_root_hash, docker_compose_hash: &docker_compose_hash };
        VmSpec {
            cpu: workload.cpus,
            ram_mib: workload.memory_mb,
            hard_disks: vec![
                HardDiskSpec { path: base_disk, format: HardDiskFormat::Qcow2, read_only: false },
                HardDiskSpec { path: verity_disk, format: HardDiskFormat::Raw, read_only: true },
                HardDiskSpec { path: state_disk_path, format: HardDiskFormat::Raw, read_only: false },
            ],
            cdrom_iso_path: Some(iso_path),
            gpus: workload.gpus.clone(),
            port_forwarding: vec![
                (workload.http_port(), 80),
                (workload.https_port(), 443),
                (workload.cvm_agent_port(), CVM_AGENT_PORT),
            ],
            bios_path: Some(cvm_config.bios),
            initrd_path: Some(cvm_config.initrd),
            kernel_path: Some(kernel.clone()),
            kernel_args: Some(kernel_args.to_string()),
            display: Default::default(),
            enable_cvm: true,
        }
    }

    async fn create_state_disk(&self, workload: &Workload) -> Result<PathBuf, StartVmError> {
        let disk_name = format!("{}.state.raw", workload.id);
        let disk_path = self.state_path.join(disk_name);
        if disk_path.exists() {
            info!("Not creating state disk because it already exists");
            return Ok(disk_path);
        }
        self.disk_service
            .create_disk(&disk_path, HardDiskFormat::Raw, workload.disk_space_gb)
            .await
            .map_err(|e| StartVmError(format!("failed to create state disk: {e}")))?;
        Ok(disk_path)
    }

    async fn create_disk_snapshot(
        &self,
        workload: &Workload,
        original_disk: &Path,
        disk_type: &str,
        disk_format: HardDiskFormat,
    ) -> Result<PathBuf, StartVmError> {
        let disk_name = format!("{}.{disk_type}.{disk_format}", workload.id);
        let disk_path = self.state_path.join(disk_name);
        if disk_path.exists() {
            info!("Not copying base image because it already exists");
            return Ok(disk_path);
        }
        self.disk_service
            .create_disk_snapshot(&disk_path, original_disk, disk_format)
            .await
            .map_err(|e| StartVmError(format!("failed to create {disk_type} disk snapshot: {e}")))?;
        Ok(disk_path)
    }

    async fn create_application_iso(&self, workload: &Workload) -> Result<(PathBuf, String), StartVmError> {
        let iso_name = format!("{}.iso", workload.id);
        let iso_path = self.state_path.join(iso_name);
        let docker_compose_hash = hex::encode(Sha256::digest(&workload.docker_compose));
        if iso_path.exists() {
            info!("Not creating ISO because it already exists");
            return Ok((iso_path, docker_compose_hash));
        }
        let environment_variables =
            workload.env_vars.iter().map(|(name, value)| EnvironmentVariable::new(name, value)).collect();
        let files = workload.files.iter().map(|(name, contents)| ExternalFile::new(name, contents.clone())).collect();
        let spec = IsoSpec {
            docker_compose_yaml: workload.docker_compose.clone(),
            metadata: ApplicationMetadata {
                hostname: workload.domain.clone(),
                api: ContainerMetadata {
                    container: workload.public_container_name.clone(),
                    port: workload.public_container_port,
                },
            },
            environment_variables,
            files,
        };
        self.disk_service
            .create_application_iso(&iso_path, spec)
            .await
            .map_err(|e| StartVmError(format!("failed to create ISO: {e}")))?;
        Ok((iso_path, docker_compose_hash))
    }
}

#[async_trait]
impl VmService for DefaultVmService {
    async fn create_vm(&self, workload: Workload) -> Result<(), StartVmError> {
        let id = workload.id;
        let socket_path = self.state_path.join(format!("{id}.sock"));
        let mut workers = self.workers.lock().await;
        match workers.get(&id) {
            Some(_) => {
                info!("VM {id} is already running");
                Ok(())
            }
            None => {
                info!("Creating disks for VM {id}");
                let spec = self.create_workload_spec(&workload).await?;
                let cvm_agent_port = workload.cvm_agent_port();
                let mut docker_credentials: Vec<_> = workload
                    .docker_credentials
                    .into_iter()
                    .map(|c| DockerCredentials { username: c.username, password: c.password, server: Some(c.server) })
                    .collect();
                docker_credentials.push(DockerCredentials {
                    username: self.docker_config.username.clone(),
                    password: self.docker_config.password.clone(),
                    server: None,
                });
                let args = VmWorkerArgs {
                    workload_id: id,
                    vm_client: self.vm_client.clone(),
                    cvm_agent_client: self.cvm_agent_client.clone(),
                    cvm_agent_port,
                    spec,
                    socket_path,
                    zerossl_config: self.zerossl_config.clone(),
                    docker_credentials,
                    event_sender: self.event_sender.clone(),
                };
                let worker = VmWorker::spawn(args);
                workers.insert(id, worker);
                Ok(())
            }
        }
    }

    async fn create_workload_spec(&self, workload: &Workload) -> Result<VmSpec, StartVmError> {
        let vm_type = if workload.gpus.is_empty() { VmType::Cpu } else { VmType::Gpu };
        let config_path = self.base_cvm_config_path.join(&workload.artifacts_version);
        let mut cvm_config = CvmConfig::from_path(&config_path, vm_type).await.map_err(|e| {
            StartVmError(format!("failed to load artifacts version {}: {e}", workload.artifacts_version))
        })?;
        let (iso_path, docker_compose_hash) = self.create_application_iso(workload).await?;
        let state_disk = self.create_state_disk(workload).await?;
        // Keep everything the same except the base disk since we'll use a snapshot
        cvm_config.vm.base_disk =
            self.create_disk_snapshot(workload, &cvm_config.vm.base_disk, "base", HardDiskFormat::Qcow2).await?;
        let spec = self.create_vm_spec(workload, iso_path, state_disk, docker_compose_hash, cvm_config);
        Ok(spec)
    }

    async fn delete_vm(&self, id: Uuid) {
        let mut workers = self.workers.lock().await;
        match workers.remove(&id) {
            Some(worker) => {
                worker.delete_vm().await;
            }
            None => {
                error!("VM {id} is not being managed by any worker");
            }
        }
    }

    async fn restart_vm(&self, id: Uuid) -> Result<(), VmNotManaged> {
        let workers = self.workers.lock().await;
        match workers.get(&id) {
            Some(worker) => {
                worker.restart_vm().await;
                Ok(())
            }
            None => {
                error!("VM {id} is not being managed by any worker");
                Err(VmNotManaged)
            }
        }
    }
}

impl CvmConfig {
    pub async fn from_path(path: &Path, vm_type: VmType) -> anyhow::Result<Self> {
        let vm_type = vm_type.to_string();
        let verity_root_hash_path = path.join(format!("vm_images/cvm-{vm_type}-verity/root-hash"));
        let verity_root_hash =
            fs::read_to_string(&verity_root_hash_path).await.context("Could not read verity hash")?.trim().into();
        let vm = CvmFiles {
            kernel: path.join(format!("vm_images/kernel/{vm_type}-vmlinuz")),
            base_disk: path.join(format!("vm_images/cvm-{vm_type}.qcow2")),
            verity_disk: path.join(format!("vm_images/cvm-{vm_type}-verity/verity-hash-dev")),
            verity_root_hash,
        };
        Ok(CvmConfig {
            initrd: path.join("initramfs/initramfs.cpio.gz"),
            bios: path.join("vm_images/ovmf/OVMF.fd"),
            vm,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CvmConfig {
    /// The path to the initrd file.
    pub initrd: PathBuf,

    /// The path to the bios file.
    pub bios: PathBuf,

    /// The disk, kernel and verity files to be used.
    pub vm: CvmFiles,
}

struct KernelArgs<'a> {
    docker_compose_hash: &'a str,
    filesystem_root_hash: &'a str,
}

impl fmt::Display for KernelArgs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { docker_compose_hash, filesystem_root_hash } = self;
        write!(f, "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={filesystem_root_hash} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={docker_compose_hash}")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("internal: {0}")]
pub struct StartVmError(String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clients::{cvm_agent::MockCvmAgentClient, qemu::MockVmClient},
        services::disk::MockDiskService,
    };
    use mockall::predicate::eq;
    use tempfile::{tempdir, TempDir};
    use tokio::sync::mpsc::channel;

    struct Context {
        service: DefaultVmService,
        #[allow(dead_code)]
        state_path: TempDir,
    }

    struct Builder {
        state_path: TempDir,
        vm_client: MockVmClient,
        cvm_agent_client: MockCvmAgentClient,
        disk_service: MockDiskService,
        base_cvm_config_path: PathBuf,
        zerossl_config: ZeroSslConfig,
        docker_config: DockerConfig,
    }

    impl Builder {
        async fn build(self) -> Context {
            let Self {
                state_path,
                vm_client,
                cvm_agent_client,
                disk_service,
                base_cvm_config_path,
                zerossl_config,
                docker_config,
            } = self;
            let args = VmServiceArgs {
                state_path: state_path.path().into(),
                vm_client: Arc::new(vm_client),
                cvm_agent_client: Arc::new(cvm_agent_client),
                disk_service: Box::new(disk_service),
                base_cvm_config_path,
                zerossl_config,
                docker_config,
                event_sender: EventSender(channel(1).0),
            };
            let service = DefaultVmService::new(args).await.expect("failed to build");
            Context { service, state_path }
        }

        async fn write_cvm_file(&self, path: &str, contents: &[u8]) {
            let path = self.base_cvm_config_path.join(path);
            let parent = path.parent().expect("no parent");
            fs::create_dir_all(parent).await.expect("failed to create parents");
            fs::write(path, contents).await.expect("failed to write contents");
        }
    }

    impl Default for Builder {
        fn default() -> Self {
            let state_path = tempdir().expect("failed to create tempdir");
            let base_path = state_path.path().to_path_buf();
            Self {
                state_path,
                vm_client: Default::default(),
                cvm_agent_client: Default::default(),
                disk_service: Default::default(),
                base_cvm_config_path: base_path.join("artifacts"),
                zerossl_config: ZeroSslConfig { eab_key_id: "key".into(), eab_mac_key: "mac".into() },
                docker_config: DockerConfig { username: "user".into(), password: "pass".into() },
            }
        }
    }

    #[tokio::test]
    async fn start_vm() {
        let workload = Workload {
            id: Uuid::new_v4(),
            docker_compose: "compose".into(),
            artifacts_version: "default".into(),
            env_vars: Default::default(),
            files: Default::default(),
            docker_credentials: Default::default(),
            public_container_name: "api".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1,
            gpus: vec![],
            disk_space_gb: 1.try_into().unwrap(),
            ports: [1000, 1001, 1002],
            domain: "example.com".into(),
            enabled: true,
        };
        let mut builder = Builder::default();
        let base_disk_contents = b"totally a disk";
        let verity_disk_contents = b"totally a disk";
        builder.write_cvm_file("default/vm_images/cvm-cpu.qcow2", base_disk_contents).await;
        builder.write_cvm_file("default/vm_images/cvm-cpu-verity/verity-hash-dev", verity_disk_contents).await;
        builder.write_cvm_file("default/vm_images/cvm-cpu-verity/root-hash", b"hash").await;

        let id = workload.id;
        let state_path = builder.state_path.path();
        let state_disk_path = state_path.join(format!("{id}.state.raw"));
        let base_disk_path = state_path.join(format!("{id}.base.qcow2"));

        builder.disk_service.expect_create_application_iso().return_once(move |_, _| Ok(()));
        builder
            .disk_service
            .expect_create_disk()
            .with(eq(state_disk_path), eq(HardDiskFormat::Raw), eq(1))
            .return_once(move |_, _, _| Ok(()));
        builder
            .disk_service
            .expect_create_disk_snapshot()
            .with(
                eq(base_disk_path.clone()),
                eq(builder.base_cvm_config_path.join("default/vm_images/cvm-cpu.qcow2")),
                eq(HardDiskFormat::Qcow2),
            )
            .return_once(move |_, _, _| Ok(()));
        builder.vm_client.expect_start_vm().return_once(move |_, _| Ok(()));

        let ctx = builder.build().await;
        ctx.service.create_vm(workload).await.expect("failed to start");
    }
}
