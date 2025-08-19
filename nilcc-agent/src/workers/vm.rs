use crate::{
    clients::{
        cvm_agent::CvmAgentClient,
        nilcc_api::{NilccApiClient, VmEvent},
        qemu::{QemuClientError, VmClient, VmSpec},
    },
    config::{DockerConfig, ZeroSslConfig},
};
use cvm_agent_models::bootstrap::{AcmeCredentials, BootstrapRequest, DockerCredentials};
use metrics::{counter, gauge};
use std::{path::PathBuf, sync::Arc, time::Duration};
use strum::EnumDiscriminants;
use tokio::{
    fs, select,
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
    time::{interval, MissedTickBehavior},
};
use tracing::{error, info, info_span, warn, Instrument};
use uuid::Uuid;

const WATCH_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq)]
pub enum InitialVmState {
    Enabled,
    Disabled,
}

pub(crate) struct VmWorkerArgs {
    pub(crate) workload_id: Uuid,
    pub(crate) vm_client: Arc<dyn VmClient>,
    pub(crate) nilcc_api_client: Arc<dyn NilccApiClient>,
    pub(crate) cvm_agent_client: Arc<dyn CvmAgentClient>,
    pub(crate) cvm_agent_port: u16,
    pub(crate) spec: VmSpec,
    pub(crate) socket_path: PathBuf,
    pub(crate) state: InitialVmState,
    pub(crate) zerossl_config: ZeroSslConfig,
    pub(crate) docker_config: DockerConfig,
}

pub(crate) struct VmWorker {
    workload_id: Uuid,
    vm_client: Arc<dyn VmClient>,
    nilcc_api_client: Arc<dyn NilccApiClient>,
    cvm_agent_client: Arc<dyn CvmAgentClient>,
    cvm_agent_port: u16,
    spec: VmSpec,
    socket_path: PathBuf,
    receiver: Receiver<WorkerCommand>,
    vm_state: VmState,
    zerossl_config: ZeroSslConfig,
    docker_config: DockerConfig,
}

impl VmWorker {
    pub(crate) fn spawn(args: VmWorkerArgs) -> VmWorkerHandle {
        let VmWorkerArgs {
            workload_id,
            vm_client,
            nilcc_api_client,
            spec,
            socket_path,
            cvm_agent_client,
            cvm_agent_port,
            state,
            zerossl_config,
            docker_config,
        } = args;
        let (sender, receiver) = channel(64);
        let join_handle = tokio::spawn(async move {
            let vm_state = match state {
                InitialVmState::Enabled => VmState::Starting,
                InitialVmState::Disabled => VmState::Disabled,
            };
            let worker = VmWorker {
                workload_id,
                vm_client,
                nilcc_api_client,
                cvm_agent_client,
                cvm_agent_port,
                spec,
                socket_path,
                receiver,
                vm_state,
                zerossl_config,
                docker_config,
            };
            worker.run().instrument(info_span!("vm_worker", workload_id = workload_id.to_string())).await;
        });
        VmWorkerHandle { sender, join_handle }
    }

    async fn run(mut self) {
        if matches!(self.vm_state, VmState::Disabled) {
            info!("Not starting VM because it's disabled");
        } else {
            self.start_vm().await;
        }

        let mut ticker = interval(WATCH_INTERVAL);
        // If we miss a tick, shift the ticks to be aligned with when we called `Interval::tick`.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            select! {
                command = self.receiver.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    self.handle_command(command).await;
                }

                _ = ticker.tick() => {
                    self.handle_tick().await
                }
            }
        }
        info!("Exiting run loop");
    }

    async fn start_vm(&mut self) {
        info!("Attempting to start VM");
        match self.vm_client.start_vm(&self.socket_path, self.spec.clone()).await {
            Ok(()) => {
                info!("VM started successfully");
                gauge!("vms_running_total").increment(1);
                self.submit_event(VmEvent::Starting).await;
                self.vm_state = VmState::Starting;
            }
            Err(QemuClientError::VmAlreadyRunning) => {
                info!("VM was already running, ignoring");
                self.vm_state = VmState::Starting;
            }
            Err(e) => {
                error!("Failed to start VM: {e}");
                counter!("vm_start_errors_total").increment(1);
                self.submit_event(VmEvent::FailedToStart { error: e.to_string() }).await;
            }
        }
    }

    async fn delete_vm(&mut self) {
        info!("Shutting down VM");
        match self.vm_client.stop_vm(&self.socket_path, true).await {
            Ok(_) => {
                info!("VM stopped")
            }
            Err(QemuClientError::VmNotRunning) => warn!("VM was not running"),
            Err(e) => {
                counter!("vm_action_errors_total", "action" => "stop").increment(1);
                error!("Failed to stop VM: {e}");
            }
        };
        // Process all non read only disks and the ISO at once
        let writeable_disks = self.spec.hard_disks.iter().filter(|d| !d.read_only);
        let paths = writeable_disks.map(|s| &s.path).chain(self.spec.cdrom_iso_path.as_ref());
        for path in paths {
            let disk_display = path.display();
            info!("Deleting disk {disk_display}");
            if let Err(e) = fs::remove_file(&path).await {
                error!("Failed to delete disk {disk_display}: {e}");
            }
        }
        self.submit_event(VmEvent::Stopped).await;
        self.vm_state = VmState::Stopped;
        gauge!("vms_running_total").decrement(1);
    }

    async fn restart_vm(&mut self) {
        info!("Shutting down VM");
        match self.vm_client.stop_vm(&self.socket_path, true).await {
            Ok(_) => {
                info!("VM is stopped and will be brought up on next tick");
            }
            Err(QemuClientError::VmNotRunning) => {
                warn!("VM was not running and will be started on next tick");
            }
            Err(e) => {
                counter!("vm_action_errors_total", "action" => "restart").increment(1);
                error!("Failed to stop VM: {e}");
            }
        }
    }

    async fn stop_vm(&mut self) {
        info!("Shutting down VM");
        match self.vm_client.stop_vm(&self.socket_path, true).await {
            Ok(_) => {
                self.vm_state = VmState::Disabled;
                self.submit_event(VmEvent::Stopped).await;
                info!("VM is stopped");
            }
            Err(QemuClientError::VmNotRunning) => {
                self.vm_state = VmState::Disabled;
                self.submit_event(VmEvent::Stopped).await;
                warn!("VM was not running");
            }
            Err(e) => {
                counter!("vm_action_errors_total", "action" => "stop").increment(1);
                error!("Failed to stop VM: {e}");
            }
        }
    }

    async fn handle_tick(&mut self) {
        if matches!(self.vm_state, VmState::Disabled) {
            info!("Ignoring tick because VM is paused");
            return;
        }

        if !self.vm_client.is_vm_running(&self.socket_path).await {
            warn!("VM is no longer running, starting it again");
            self.submit_event(VmEvent::Stopped).await;
            self.start_vm().await;
            return;
        }

        if !matches!(self.vm_state, VmState::Running) {
            info!("Checking health of CVM agent");
            match self.cvm_agent_client.check_health(self.cvm_agent_port).await {
                Ok(response) => {
                    if !response.bootstrapped {
                        info!("CVM agent is running, bootstrapping it");
                        let request = BootstrapRequest {
                            // TODO: these are deprecated and should be removed once all active
                            // cvms are migrated
                            acme_eab_key_id: self.zerossl_config.eab_key_id.clone(),
                            acme_eab_mac_key: self.zerossl_config.eab_mac_key.clone(),
                            acme: AcmeCredentials {
                                eab_key_id: self.zerossl_config.eab_key_id.clone(),
                                eab_mac_key: self.zerossl_config.eab_mac_key.clone(),
                            },
                            docker: vec![DockerCredentials {
                                username: self.docker_config.username.clone(),
                                password: self.docker_config.password.clone(),
                                server: None,
                            }],
                        };
                        if let Err(e) = self.cvm_agent_client.bootstrap(self.cvm_agent_port, &request).await {
                            warn!("Failed to bootstrap agent: {e:#}");
                            return;
                        }
                        info!("CVM agent is bootstrapped");
                    }
                    if response.https {
                        info!("CVM's https endpoint is functional");
                        self.vm_state = VmState::Running;
                        self.submit_event(VmEvent::Running).await;
                    }
                }
                Err(e) => {
                    warn!("Failed to check CVM agent health: {e:#}");
                }
            }
        }
    }

    async fn handle_command(&mut self, command: WorkerCommand) {
        let discriminant = WorkerCommandDiscriminants::from(&command);
        info!("Received {discriminant:?} command");
        match command {
            WorkerCommand::Delete => self.delete_vm().await,
            WorkerCommand::Restart => self.restart_vm().await,
            WorkerCommand::Stop => self.stop_vm().await,
            WorkerCommand::Start => self.start_vm().await,
        }
    }

    async fn submit_event(&self, event: VmEvent) {
        info!("Submitting event to API");
        if let Err(e) = self.nilcc_api_client.report_vm_event(self.workload_id, event).await {
            error!("Failed to submit event to API: {e:#}");
        }
    }
}

enum VmState {
    Starting,
    Running,
    Stopped,
    Disabled,
}

pub(crate) struct VmWorkerHandle {
    sender: Sender<WorkerCommand>,
    #[allow(dead_code)]
    join_handle: JoinHandle<()>,
}

impl VmWorkerHandle {
    pub(crate) async fn delete_vm(&self) {
        self.send_command(WorkerCommand::Delete).await;
    }

    pub(crate) async fn restart_vm(&self) {
        self.send_command(WorkerCommand::Restart).await;
    }

    pub(crate) async fn stop_vm(&self) {
        self.send_command(WorkerCommand::Stop).await;
    }

    pub(crate) async fn start_vm(&self) {
        self.send_command(WorkerCommand::Start).await;
    }

    async fn send_command(&self, command: WorkerCommand) {
        if self.sender.send(command).await.is_err() {
            error!("Worker receiver dropped");
        }
    }
}

#[derive(Debug, EnumDiscriminants)]
enum WorkerCommand {
    Delete,
    Restart,
    Stop,
    Start,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::{cvm_agent::MockCvmAgentClient, nilcc_api::MockNilccApiClient, qemu::MockVmClient};
    use mockall::predicate::eq;

    fn make_spec() -> VmSpec {
        VmSpec {
            cpu: 1,
            ram_mib: 1024,
            hard_disks: vec![],
            cdrom_iso_path: None,
            gpus: vec![],
            port_forwarding: vec![],
            bios_path: None,
            initrd_path: None,
            kernel_path: None,
            kernel_args: None,
            display: Default::default(),
            enable_cvm: false,
        }
    }

    #[tokio::test]
    async fn start_stop() {
        let id = Uuid::new_v4();
        let spec = make_spec();
        let socket = PathBuf::from("/tmp/vm.sock");
        let mut vm_client = MockVmClient::default();
        let mut nilcc_api_client = MockNilccApiClient::default();
        vm_client.expect_start_vm().with(eq(socket.clone()), eq(spec.clone())).once().return_once(move |_, _| Ok(()));
        vm_client.expect_stop_vm().with(eq(socket.clone()), eq(true)).once().return_once(move |_, _| Ok(()));

        nilcc_api_client
            .expect_report_vm_event()
            .with(eq(id), eq(VmEvent::Starting))
            .once()
            .return_once(move |_, _| Ok(()));
        nilcc_api_client
            .expect_report_vm_event()
            .with(eq(id), eq(VmEvent::Stopped))
            .once()
            .return_once(move |_, _| Ok(()));

        let args = VmWorkerArgs {
            workload_id: id,
            vm_client: Arc::new(vm_client),
            nilcc_api_client: Arc::new(nilcc_api_client),
            cvm_agent_client: Arc::new(MockCvmAgentClient::default()),
            cvm_agent_port: 5555,
            spec,
            socket_path: socket,
            state: InitialVmState::Enabled,
            zerossl_config: ZeroSslConfig { eab_key_id: "key".into(), eab_mac_key: "mac".into() },
            docker_config: DockerConfig { username: "user".into(), password: "pass".into() },
        };
        let join_handle = {
            let handle = VmWorker::spawn(args);
            handle.delete_vm().await;
            handle.join_handle
        };
        join_handle.await.expect("failed to join");
    }
}
