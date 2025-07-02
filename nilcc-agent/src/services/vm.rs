use crate::{
    clients::qemu::VmClient,
    repositories::workload::{WorkloadModel, WorkloadRepository},
    services::sni_proxy::SniProxyService,
};
use anyhow::Context;
use async_trait::async_trait;
use metrics::{counter, gauge};
use std::{path::PathBuf, sync::Arc, time::Duration};
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
    pub workload_repository: Arc<dyn WorkloadRepository>,
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
    workload_repository: Arc<dyn WorkloadRepository>,
    sni_proxy_service: Box<dyn SniProxyService>,
    receiver: Receiver<Command>,
}

impl Worker {
    fn start(args: VmServiceArgs) -> Sender<Command> {
        let (sender, receiver) = channel(WORKER_CHANNEL_SIZE);
        let VmServiceArgs { state_path, vm_client, workload_repository, sni_proxy_service } = args;
        tokio::spawn(async move {
            let worker = Worker { state_path, vm_client, workload_repository, sni_proxy_service, receiver };
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

    async fn handle_command(&mut self, _command: Command) -> anyhow::Result<()> {
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

    fn socket_path(&self, vm_id: Uuid) -> PathBuf {
        let file_name = format!("{vm_id}.sock");
        self.state_path.join(file_name)
    }

    async fn update_sni_proxy_config(&self) -> anyhow::Result<()> {
        info!("Updating SNI proxy configuration");
        let workloads = self.workload_repository.list().await?;
        self.sni_proxy_service.update_config(workloads).await.context("Failed to update SNI proxy configuration")
    }
}

#[allow(dead_code)]
enum Command {
    SyncVm { workload: WorkloadModel },
    StopVm { id: Uuid },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clients::qemu::MockVmClient, repositories::workload::MockWorkloadRepository,
        services::sni_proxy::MockSniProxyService,
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
        workload_repository: MockWorkloadRepository,
        sni_proxy_service: MockSniProxyService,
    }

    impl WorkerBuilder {
        fn build(self) -> WorkerCtx {
            let Self { state_path, vm_client, workload_repository, sni_proxy_service } = self;
            WorkerCtx {
                worker: Worker {
                    state_path: state_path.path().into(),
                    vm_client: Box::new(vm_client),
                    workload_repository: Arc::new(workload_repository),
                    sni_proxy_service: Box::new(sni_proxy_service),
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
                workload_repository: Default::default(),
                sni_proxy_service: Default::default(),
            }
        }
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
        let workloads = vec![make_workload(), make_workload()];
        builder
            .sni_proxy_service
            .expect_update_config()
            .with(eq(workloads.clone()))
            .once()
            .return_once(move |_| Ok(()));
        builder.workload_repository.expect_list().return_once(move || Ok(workloads));

        let mut ctx = builder.build();
        ctx.worker.handle_command(Command::StopVm { id }).await.expect("failed to handle command");
    }
}
