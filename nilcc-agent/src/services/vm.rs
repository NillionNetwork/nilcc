use crate::{
    config::CvmConfig,
    iso::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoSpec},
    qemu_client::{HardDiskFormat, HardDiskSpec, QemuClientError, VmClient, VmSpec},
    repositories::workload::WorkloadModel,
    services::disk::{DiskService, DockerComposeHash},
};
use anyhow::Context;
use async_trait::async_trait;
use std::{fmt, path::PathBuf};
use tokio::{
    fs,
    sync::mpsc::{channel, Receiver, Sender},
};
use tracing::{error, info, warn};
use uuid::Uuid;

const WORKER_CHANNEL_SIZE: usize = 1024;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmService: Send + Sync {
    /// Synchronize a VM's to the given workload.
    async fn sync_vm(&self, workload: WorkloadModel);

    /// Stop a VM and delete all associated resources.
    async fn stop_vm(&self, id: Uuid);
}

pub struct DefaultVmService {
    worker_sender: Sender<Command>,
}

impl DefaultVmService {
    pub async fn new(
        state_path: PathBuf,
        vm_client: Box<dyn VmClient>,
        disk_service: Box<dyn DiskService>,
        cvm_config: CvmConfig,
    ) -> anyhow::Result<Self> {
        fs::create_dir_all(&state_path).await.context("Failed to create state path")?;

        let worker_sender = Worker::start(state_path, vm_client, disk_service, cvm_config);
        Ok(Self { worker_sender })
    }

    async fn send_worker(&self, command: Command) {
        if self.worker_sender.send(command).await.is_err() {
            error!("Worker channel dropped");
        }
    }
}

#[async_trait]
impl VmService for DefaultVmService {
    async fn sync_vm(&self, workload: WorkloadModel) {
        self.send_worker(Command::SyncVm { workload }).await;
    }

    async fn stop_vm(&self, id: Uuid) {
        self.send_worker(Command::StopVm { id }).await;
    }
}

struct Worker {
    state_path: PathBuf,
    vm_client: Box<dyn VmClient>,
    disk_service: Box<dyn DiskService>,
    cvm_config: CvmConfig,
    receiver: Receiver<Command>,
}

impl Worker {
    fn start(
        state_path: PathBuf,
        vm_client: Box<dyn VmClient>,
        disk_service: Box<dyn DiskService>,
        cvm_config: CvmConfig,
    ) -> Sender<Command> {
        let (sender, receiver) = channel(WORKER_CHANNEL_SIZE);
        tokio::spawn(async move {
            let worker = Worker { state_path, vm_client, disk_service, cvm_config, receiver };
            worker.run().await;
            warn!("Worker loop exited");
        });
        sender
    }

    async fn run(mut self) {
        while let Some(command) = self.receiver.recv().await {
            let result = match command {
                Command::SyncVm { workload } => {
                    info!("Need to sync vm {}", workload.id);
                    self.sync_vm(workload).await
                }
                Command::StopVm { id } => {
                    info!("Need to stop vm {id}");
                    self.stop_vm(id).await
                }
            };
            if let Err(e) = result {
                error!("Failed to run command: {e}")
            }
        }
    }

    async fn sync_vm(&mut self, workload: WorkloadModel) -> anyhow::Result<()> {
        let id = workload.id;
        let socket_path = self.socket_path(id);
        info!("Trying to stop VM {id} before syncing");
        match self.vm_client.stop_vm(&socket_path, true).await {
            Ok(()) => (),
            Err(QemuClientError::VmNotRunning) => {
                info!("VM was not running");
            }
            Err(e) => Err(e).context("Failed to stop running VM")?,
        };

        info!("Starting VM {id}");
        let (iso_path, docker_compose_hash) = self.create_application_iso(&workload).await?;
        let state_disk = self.create_state_disk(&workload).await?;
        let vm_spec = self.create_vm_spec(&workload, iso_path, state_disk, docker_compose_hash)?;
        self.vm_client.start_vm(vm_spec, &socket_path).await.context("Failed to start VM")?;
        Ok(())
    }

    async fn stop_vm(&mut self, id: Uuid) -> anyhow::Result<()> {
        let socket_path = self.socket_path(id);
        info!("Stopping VM {id}");
        match self.vm_client.stop_vm(&socket_path, true).await {
            Ok(()) => Ok(()),
            Err(QemuClientError::VmNotRunning) => {
                info!("VM was not running");
                Ok(())
            }
            Err(e) => Err(e).context("Failed to stop running VM")?,
        }
    }

    fn socket_path(&self, vm_id: Uuid) -> PathBuf {
        let file_name = format!("{vm_id}.sock");
        self.state_path.join(file_name)
    }

    async fn create_state_disk(&self, workload: &WorkloadModel) -> anyhow::Result<PathBuf> {
        let disk_name = format!("{}.raw", workload.id);
        let disk_path = self.state_path.join(disk_name);
        self.disk_service
            .create_disk(&disk_path, HardDiskFormat::Raw, workload.disk_gb.into())
            .await
            .context("Failed to create state disk")?;
        Ok(disk_path)
    }

    async fn create_application_iso(&self, workload: &WorkloadModel) -> anyhow::Result<(PathBuf, DockerComposeHash)> {
        let iso_name = format!("{}.iso", workload.id);
        let iso_path = self.state_path.join(iso_name);
        let environment_variables =
            workload.environment_variables.iter().map(|(name, value)| EnvironmentVariable::new(name, value)).collect();
        let spec = IsoSpec {
            docker_compose_yaml: workload.docker_compose.clone(),
            metadata: ApplicationMetadata {
                hostname: "UNKNOWN".into(),
                api: ContainerMetadata {
                    container: workload.public_container_name.clone(),
                    port: workload.public_container_port,
                },
            },
            environment_variables,
        };
        let docker_compose_hash = self
            .disk_service
            .create_application_iso(&iso_path, spec)
            .await
            .context("Failed to create application ISO")?;
        Ok((iso_path, docker_compose_hash))
    }

    fn create_vm_spec(
        &self,
        workload: &WorkloadModel,
        iso_path: PathBuf,
        state_disk_path: PathBuf,
        docker_compose_hash: DockerComposeHash,
    ) -> anyhow::Result<VmSpec> {
        let kernel_args = KernelArgs {
            filesystem_root_hash: &self.cvm_config.verity_root_hash,
            docker_compose_hash: &docker_compose_hash.0,
        };
        let spec = VmSpec {
            cpu: workload.cpus.into(),
            ram_mib: workload.memory_mb,
            hard_disks: vec![
                HardDiskSpec { path: self.cvm_config.base_disk.clone(), format: HardDiskFormat::Qcow2 },
                HardDiskSpec { path: self.cvm_config.verity_disk.clone(), format: HardDiskFormat::Raw },
                HardDiskSpec { path: state_disk_path, format: HardDiskFormat::Raw },
            ],
            cdrom_iso_path: Some(iso_path),
            gpu_enabled: false,
            port_forwarding: vec![(workload.metal_http_port, 80), (workload.metal_https_port, 443)],
            bios_path: Some(self.cvm_config.bios.clone()),
            initrd_path: Some(self.cvm_config.initrd.clone()),
            kernel_path: Some(self.cvm_config.kernel.clone()),
            kernel_args: Some(kernel_args.to_string()),
            display_gtk: false,
            enable_cvm: true,
        };
        Ok(spec)
    }
}

enum Command {
    SyncVm { workload: WorkloadModel },
    StopVm { id: Uuid },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{qemu_client::MockVmClient, services::disk::MockDiskService};
    use mockall::predicate::eq;
    use tempfile::TempDir;

    fn make_workload() -> WorkloadModel {
        WorkloadModel {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            environment_variables: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_gb: 1.try_into().unwrap(),
            gpus: Default::default(),
            metal_http_port: 1080,
            metal_https_port: 1443,
        }
    }

    struct WorkerCtx {
        worker: Worker,
        #[allow(dead_code)]
        state_path: TempDir,
    }

    struct WorkerBuilder {
        state_path: TempDir,
        vm_client: MockVmClient,
        disk_service: MockDiskService,
        cvm_config: CvmConfig,
    }

    impl WorkerBuilder {
        fn build(self) -> WorkerCtx {
            let Self { state_path, vm_client, disk_service, cvm_config } = self;
            WorkerCtx {
                worker: Worker {
                    state_path: state_path.path().into(),
                    vm_client: Box::new(vm_client),
                    disk_service: Box::new(disk_service),
                    cvm_config,
                    receiver: channel(1).1,
                },
                state_path,
            }
        }
    }

    impl Default for WorkerBuilder {
        fn default() -> Self {
            Self {
                state_path: tempfile::tempdir().expect("failed to create temp dir"),
                vm_client: Default::default(),
                disk_service: Default::default(),
                cvm_config: CvmConfig {
                    initrd: "/initrd".into(),
                    kernel: "/kernel".into(),
                    bios: "/bios".into(),
                    base_disk: "/base_disk".into(),
                    verity_disk: "/verity_disk".into(),
                    verity_root_hash: "root-hash".into(),
                },
            }
        }
    }

    #[tokio::test]
    async fn application_iso() {
        let mut builder = WorkerBuilder::default();
        let id = Uuid::new_v4();
        let expected_iso_path = builder.state_path.path().join(format!("{id}.iso"));
        let workload = WorkloadModel {
            id,
            docker_compose: "some_yaml".into(),
            public_container_name: "foo".into(),
            public_container_port: 80,
            environment_variables: [("var".into(), "value".into())].into(),
            ..make_workload()
        };
        let spec = IsoSpec {
            docker_compose_yaml: workload.docker_compose.clone(),
            metadata: ApplicationMetadata {
                hostname: "UNKNOWN".into(),
                api: ContainerMetadata {
                    container: workload.public_container_name.clone(),
                    port: workload.public_container_port,
                },
            },
            environment_variables: vec![EnvironmentVariable::new("var", "value")],
        };

        builder
            .disk_service
            .expect_create_application_iso()
            .with(eq(expected_iso_path.clone()), eq(spec))
            .return_once(move |_, _| Ok(DockerComposeHash("hash".into())));

        let ctx = builder.build();
        let (iso_path, _) = ctx.worker.create_application_iso(&workload).await.expect("failed to generate ISO");
        assert_eq!(iso_path, expected_iso_path);
    }

    #[test]
    fn vm_spec() {
        let builder = WorkerBuilder::default();
        let docker_compose_hash = "deadbeef";
        let workload = WorkloadModel { memory_mb: 1024, cpus: 2.try_into().unwrap(), gpus: 0, ..make_workload() };
        let iso_path = PathBuf::from("/tmp/vm.iso");
        let filesystem_root_hash = &builder.cvm_config.verity_root_hash;
        let kernel_args = KernelArgs { docker_compose_hash, filesystem_root_hash }.to_string();
        let state_disk_path = PathBuf::from("/tmp/vm.raw");

        let expected_spec = VmSpec {
            cpu: 2,
            ram_mib: 1024,
            hard_disks: vec![
                HardDiskSpec { path: "/base_disk".into(), format: HardDiskFormat::Qcow2 },
                HardDiskSpec { path: "/verity_disk".into(), format: HardDiskFormat::Raw },
                HardDiskSpec { path: state_disk_path.clone(), format: HardDiskFormat::Raw },
            ],
            cdrom_iso_path: Some(iso_path.clone()),
            gpu_enabled: false,
            port_forwarding: vec![(workload.metal_http_port, 80), (workload.metal_https_port, 443)],
            bios_path: Some("/bios".into()),
            initrd_path: Some("/initrd".into()),
            kernel_path: Some("/kernel".into()),
            kernel_args: Some(kernel_args),
            display_gtk: false,
            enable_cvm: true,
        };
        let ctx = builder.build();
        let spec = ctx
            .worker
            .create_vm_spec(&workload, iso_path, state_disk_path, DockerComposeHash(docker_compose_hash.to_string()))
            .expect("failed to create vm spec");

        assert_eq!(spec, expected_spec);
    }
}
