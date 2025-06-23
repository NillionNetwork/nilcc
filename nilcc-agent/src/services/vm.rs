use crate::{
    config::CvmConfig,
    iso::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoSpec},
    qemu_client::{HardDiskFormat, HardDiskSpec, QemuClientError, VmClient, VmSpec},
    repositories::workload::{WorkloadModel, WorkloadModelStatus, WorkloadRepository},
    services::{
        disk::{DiskService, DockerComposeHash},
        sni_proxy::SniProxyService,
    },
};
use anyhow::Context;
use async_trait::async_trait;
use metrics::{counter, gauge};
use std::{fmt, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    fs, select,
    sync::mpsc::{channel, Receiver, Sender},
    time::{interval, MissedTickBehavior},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const WORKER_CHANNEL_SIZE: usize = 1024;
const WATCH_INTERVAL: Duration = Duration::from_secs(10);

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmService: Send + Sync {
    /// Synchronize a VM's to the given workload.
    async fn sync_vm(&self, workload: WorkloadModel);

    /// Stop a VM and delete all associated resources.
    async fn stop_vm(&self, id: Uuid);
}

pub struct VmServiceArgs {
    pub state_path: PathBuf,
    pub vm_client: Box<dyn VmClient>,
    pub disk_service: Box<dyn DiskService>,
    pub workload_repository: Arc<dyn WorkloadRepository>,
    pub cvm_config: CvmConfig,
    pub sni_proxy_service: Box<dyn SniProxyService>,
}

pub struct DefaultVmService {
    worker_sender: Sender<Command>,
}

impl DefaultVmService {
    pub async fn new(args: VmServiceArgs) -> anyhow::Result<Self> {
        fs::create_dir_all(&args.state_path).await.context("Failed to create state path")?;

        let worker_sender = Worker::start(args);
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
    workload_repository: Arc<dyn WorkloadRepository>,
    sni_proxy_service: Box<dyn SniProxyService>,
    cvm_config: CvmConfig,
    receiver: Receiver<Command>,
}

impl Worker {
    fn start(args: VmServiceArgs) -> Sender<Command> {
        let (sender, receiver) = channel(WORKER_CHANNEL_SIZE);
        let VmServiceArgs { state_path, vm_client, disk_service, workload_repository, cvm_config, sni_proxy_service } =
            args;
        tokio::spawn(async move {
            let worker = Worker {
                state_path,
                vm_client,
                disk_service,
                workload_repository,
                sni_proxy_service,
                cvm_config,
                receiver,
            };
            worker.run().await;
            warn!("Worker loop exited");
        });
        sender
    }

    async fn run(mut self) {
        let mut ticker = interval(WATCH_INTERVAL);
        // If we miss a tick, shift the ticks to be aligned with when we called `Interval::tick`.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            select! {
                command = self.receiver.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    if let Err(e) = self.handle_command(command).await {
                        counter!("vm_commands_failed_total").increment(1);
                        error!("Failed to run command: {e}")
                    }
                },
                _ = ticker.tick() => {
                    if let Err(e) = self.check_running_vms().await {
                        counter!("vm_check_running_vms_errors_total").increment(1);
                        error!("Failed to check running VMs: {e}");
                    }
                }
            };
        }
        warn!("Worker loop ended");
    }

    async fn handle_command(&mut self, command: Command) -> anyhow::Result<()> {
        match command {
            Command::SyncVm { workload } => {
                info!("Need to sync vm {}", workload.id);
                counter!("vm_commands_executed_total", "command" => "start").increment(1);
                self.sync_vm(workload).await?;
            }
            Command::StopVm { id } => {
                info!("Need to stop vm {id}");
                counter!("vm_commands_executed_total", "command" => "stop").increment(1);
                self.stop_vm(id).await?;
            }
        };
        self.update_sni_proxy_config().await
    }

    async fn check_running_vms(&self) -> anyhow::Result<()> {
        info!("Checking which VMs are running");
        let mut running = 0;
        let workloads = self.workload_repository.list().await.context("Failed to find workloads")?;
        let expected = workloads.len();
        let mut total_cpus: u16 = 0;
        let mut total_gpus: u16 = 0;
        let mut total_memory: u32 = 0;
        for workload in workloads {
            let id = workload.id;
            let socket_path = self.socket_path(id);
            debug!("Checking if VM {id} is running");
            if self.vm_client.is_vm_running(&socket_path).await {
                running += 1;
            } else {
                warn!("VM {id} is not running, removing it");
                if let Err(e) = self.workload_repository.delete(id).await {
                    error!("Failed to delete VM {id}: {e}");
                }
            }
            total_cpus = total_cpus.saturating_add(workload.cpus.into());
            total_gpus = total_gpus.saturating_add(workload.gpus.len() as u16);
            total_memory = total_memory.saturating_add(workload.memory_mb);
        }
        info!("{running}/{expected} machines are running");
        gauge!("vms_total", "type" => "running").set(running);
        gauge!("vms_total", "type" => "desired").set(expected as u32);
        gauge!("vms_resources_used_total", "resource" => "cpu").set(total_cpus);
        gauge!("vms_resources_used_total", "resource" => "gpu").set(total_gpus);
        gauge!("vms_resources_used_total", "resource" => "memory_mb").set(total_memory);
        Ok(())
    }

    async fn sync_vm(&mut self, mut workload: WorkloadModel) -> anyhow::Result<()> {
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

        // Mark it as running and update it
        workload.status = WorkloadModelStatus::Running;
        self.workload_repository.upsert(workload).await.context("Failed to upsert workload")?;
        Ok(())
    }

    async fn stop_vm(&mut self, id: Uuid) -> anyhow::Result<()> {
        let socket_path = self.socket_path(id);
        info!("Stopping VM {id}");
        match self.vm_client.stop_vm(&socket_path, true).await {
            Ok(()) => (),
            Err(QemuClientError::VmNotRunning) => {
                info!("VM was not running");
            }
            Err(e) => return Err(e).context("Failed to stop running VM")?,
        };
        self.workload_repository.delete(id).await.context("Failed to delete workload")?;
        Ok(())
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
            gpus: workload.gpus.clone(),
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

    async fn update_sni_proxy_config(&self) -> anyhow::Result<()> {
        info!("Updating SNI proxy configuration");
        let workloads = self.workload_repository.list().await?;
        self.sni_proxy_service.update_config(workloads).await.context("Failed to update SNI proxy configuration")
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
    use crate::{
        qemu_client::MockVmClient,
        repositories::workload::MockWorkloadRepository,
        services::{disk::MockDiskService, sni_proxy::MockSniProxyService},
    };
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
            status: Default::default(),
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
        workload_repository: MockWorkloadRepository,
        sni_proxy_service: MockSniProxyService,
        cvm_config: CvmConfig,
    }

    impl WorkerBuilder {
        fn build(self) -> WorkerCtx {
            let Self { state_path, vm_client, disk_service, workload_repository, sni_proxy_service, cvm_config } = self;
            WorkerCtx {
                worker: Worker {
                    state_path: state_path.path().into(),
                    vm_client: Box::new(vm_client),
                    disk_service: Box::new(disk_service),
                    workload_repository: Arc::new(workload_repository),
                    sni_proxy_service: Box::new(sni_proxy_service),
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
                workload_repository: Default::default(),
                sni_proxy_service: Default::default(),
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
        let workload = WorkloadModel {
            memory_mb: 1024,
            cpus: 2.try_into().unwrap(),
            gpus: vec!["addr".into()],
            ..make_workload()
        };
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
            gpus: vec!["addr".into()],
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

    #[tokio::test]
    async fn check_running_vms() {
        let mut builder = WorkerBuilder::default();
        // We should be running 2, one will be running, one will be stopped
        let running_id = Uuid::new_v4();
        let stopped_id = Uuid::new_v4();
        let base_path = builder.state_path.path().display();
        let running_socket = PathBuf::from(format!("{base_path}/{running_id}.sock"));
        let stopped_socket = PathBuf::from(format!("{base_path}/{stopped_id}.sock"));
        let workloads = vec![
            WorkloadModel { id: running_id, ..make_workload() },
            WorkloadModel { id: stopped_id, ..make_workload() },
        ];
        builder.workload_repository.expect_list().return_once(move || Ok(workloads));

        // Claim they're running/stopped
        builder.vm_client.expect_is_vm_running().with(eq(running_socket)).return_once(move |_| true);
        builder.vm_client.expect_is_vm_running().with(eq(stopped_socket)).return_once(move |_| false);

        // Expect to delete them
        builder.workload_repository.expect_delete().with(eq(stopped_id)).return_once(move |_| Ok(()));
    }

    #[tokio::test]
    async fn stop_vm_command() {
        let id = Uuid::new_v4();
        let mut builder = WorkerBuilder::default();
        let path = builder.state_path.path().join(format!("{id}.sock"));
        let workloads = vec![make_workload(), make_workload()];
        builder
            .sni_proxy_service
            .expect_update_config()
            .with(eq(workloads.clone()))
            .once()
            .return_once(move |_| Ok(()));
        builder.workload_repository.expect_list().return_once(move || Ok(workloads));
        builder.vm_client.expect_stop_vm().with(eq(path), eq(true)).once().return_once(move |_, _| Ok(()));
        builder.workload_repository.expect_delete().with(eq(id)).once().return_once(move |_| Ok(()));

        let mut ctx = builder.build();
        ctx.worker.handle_command(Command::StopVm { id }).await.expect("failed to handle command");
    }
}
