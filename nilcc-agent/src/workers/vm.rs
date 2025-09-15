use crate::{
    clients::{
        cvm_agent::CvmAgentClient,
        nilcc_api::VmEvent,
        qemu::{QemuClientError, VmClient, VmSpec},
    },
    config::ZeroSslConfig,
    workers::events::EventSender,
};
use chrono::Utc;
use cvm_agent_models::{
    bootstrap::{AcmeCredentials, BootstrapRequest, DockerCredentials},
    health::{EventKind, LastEvent},
};
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

pub(crate) struct VmWorkerArgs {
    pub(crate) workload_id: Uuid,
    pub(crate) vm_client: Arc<dyn VmClient>,
    pub(crate) cvm_agent_client: Arc<dyn CvmAgentClient>,
    pub(crate) cvm_agent_port: u16,
    pub(crate) spec: VmSpec,
    pub(crate) socket_path: PathBuf,
    pub(crate) zerossl_config: ZeroSslConfig,
    pub(crate) docker_credentials: Vec<DockerCredentials>,
    pub(crate) event_sender: EventSender,
    pub(crate) domain: String,
}

pub(crate) struct VmWorker {
    workload_id: Uuid,
    vm_client: Arc<dyn VmClient>,
    cvm_agent_client: Arc<dyn CvmAgentClient>,
    cvm_agent_port: u16,
    spec: VmSpec,
    socket_path: PathBuf,
    receiver: Receiver<WorkerCommand>,
    vm_state: VmState,
    zerossl_config: ZeroSslConfig,
    docker_credentials: Vec<DockerCredentials>,
    domain: String,
    event_sender: EventSender,
    last_event_id: Option<u64>,
}

impl VmWorker {
    pub(crate) fn spawn(args: VmWorkerArgs) -> VmWorkerHandle {
        let VmWorkerArgs {
            workload_id,
            vm_client,
            spec,
            socket_path,
            cvm_agent_client,
            cvm_agent_port,
            zerossl_config,
            docker_credentials,
            event_sender,
            domain,
        } = args;
        let (sender, receiver) = channel(64);
        let join_handle = tokio::spawn(async move {
            let worker = VmWorker {
                workload_id,
                vm_client,
                cvm_agent_client,
                cvm_agent_port,
                spec,
                socket_path,
                receiver,
                vm_state: Default::default(),
                zerossl_config,
                docker_credentials,
                event_sender,
                domain,
                last_event_id: None,
            };
            worker.run().instrument(info_span!("vm_worker", workload_id = workload_id.to_string())).await;
        });
        VmWorkerHandle { sender, join_handle }
    }

    async fn run(mut self) {
        self.start_vm().await;

        let mut ticker = interval(WATCH_INTERVAL);
        // If we miss a tick, shift the ticks to be aligned with when we called `Interval::tick`.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            select! {
                command = self.receiver.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    let needs_break = matches!(command, WorkerCommand::Delete);
                    self.handle_command(command).await;
                    if needs_break {
                        break;
                    }
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
        if let Err(e) = fs::remove_file(&self.socket_path).await {
            warn!("Failed to delete qemu socket: {e}");
        }
        gauge!("vms_running_total").decrement(1);
    }

    async fn restart_vm(&mut self) {
        info!("Shutting down VM because we want an explicit restart");
        self.submit_event(VmEvent::ForcedRestart).await;
        match self.vm_client.stop_vm(&self.socket_path, true).await {
            Ok(_) => {
                self.start_vm().await;
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

    async fn handle_tick(&mut self) {
        if !self.vm_client.is_vm_running(&self.socket_path).await {
            warn!("VM is no longer running, starting it again");
            self.submit_event(VmEvent::VmRestarted).await;
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
                            docker: self.docker_credentials.clone(),
                            domain: self.domain.clone(),
                        };
                        if let Err(e) = self.cvm_agent_client.bootstrap(self.cvm_agent_port, &request).await {
                            warn!("Failed to bootstrap agent: {e:#}");
                            return;
                        }
                        self.submit_event(VmEvent::AwaitingCert).await;
                        info!("CVM agent is bootstrapped");
                    }
                    if response.https {
                        info!("CVM's https endpoint is functional");
                        self.vm_state = VmState::Running;
                        self.submit_event(VmEvent::Running).await;
                    }
                    if let Some(last_event) = response.last_event {
                        let LastEvent { id, kind, message, timestamp } = last_event;
                        if self.last_event_id != Some(id) {
                            info!("CVM reported {kind:?} event: {message}");
                            self.last_event_id = Some(id);

                            let event = match &kind {
                                EventKind::Error => VmEvent::FailedToStart { error: message },
                                EventKind::Warning => VmEvent::Warning { message },
                            };
                            self.event_sender.send_event(self.workload_id, event, timestamp).await;
                        }
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
        }
    }

    async fn submit_event(&self, event: VmEvent) {
        let timestamp = Utc::now();
        self.event_sender.send_event(self.workload_id, event, timestamp).await;
    }
}

#[derive(Default)]
enum VmState {
    #[default]
    Unknown,
    Starting,
    Running,
    Stopped,
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
}
