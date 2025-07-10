use crate::clients::{
    nilcc_api::{NilccApiClient, VmEvent},
    qemu::{QemuClientError, VmClient, VmSpec},
};
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

pub(crate) struct VmWorker {
    workload_id: Uuid,
    vm_client: Arc<dyn VmClient>,
    nilcc_api_client: Arc<dyn NilccApiClient>,
    spec: VmSpec,
    socket_path: PathBuf,
    receiver: Receiver<WorkerCommand>,
}

impl VmWorker {
    pub(crate) fn spawn(
        workload_id: Uuid,
        vm_client: Arc<dyn VmClient>,
        nilcc_api_client: Arc<dyn NilccApiClient>,
        spec: VmSpec,
        socket_path: PathBuf,
    ) -> VmWorkerHandle {
        let (sender, receiver) = channel(64);
        let join_handle = tokio::spawn(async move {
            let worker = VmWorker { workload_id, vm_client, nilcc_api_client, spec, socket_path, receiver };
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
                    self.handle_command(command).await;
                }

                _ = ticker.tick() => {
                    self.handle_tick().await
                }
            }
        }
        info!("Exiting run loop");
    }

    async fn start_vm(&self) {
        info!("Attempting to start VM");
        match self.vm_client.start_vm(self.spec.clone(), &self.socket_path).await {
            Ok(()) => {
                info!("VM started successfully");
                self.submit_event(VmEvent::Started).await;
            }
            Err(QemuClientError::VmAlreadyRunning) => {
                info!("VM was already running, ignoring");
            }
            Err(e) => {
                error!("Failed to start VM: {e}");
                self.submit_event(VmEvent::FailedToStart { error: e.to_string() }).await;
            }
        }
    }

    async fn delete_vm(&self) {
        match self.vm_client.stop_vm(&self.socket_path, true).await {
            Ok(_) => {
                // Process all disks and the ISO at once
                let paths = self.spec.hard_disks.iter().map(|s| &s.path).chain(self.spec.cdrom_iso_path.as_ref());
                for path in paths {
                    let disk_display = path.display();
                    info!("Deleting disk {disk_display}");
                    if let Err(e) = fs::remove_file(&path).await {
                        error!("Failed to delete disk {disk_display}: {e}");
                    }
                }
                self.submit_event(VmEvent::Stopped).await;
                info!("VM stopped")
            }
            Err(QemuClientError::VmNotRunning) => warn!("VM was not running"),
            Err(e) => {
                error!("Failed to stop VM: {e}");
            }
        }
    }

    async fn handle_tick(&self) {
        if self.vm_client.is_vm_running(&self.socket_path).await {
            return;
        }
        warn!("VM is no longer running, starting it again");
        self.submit_event(VmEvent::Stopped).await;
        self.start_vm().await;
    }

    async fn handle_command(&self, command: WorkerCommand) {
        let discriminant = WorkerCommandDiscriminants::from(&command);
        info!("Received {discriminant:?} command");
        match command {
            WorkerCommand::DeleteVm => self.delete_vm().await,
        }
    }

    async fn submit_event(&self, event: VmEvent) {
        info!("Submitting event to API");
        if let Err(e) = self.nilcc_api_client.report_vm_event(self.workload_id, event).await {
            error!("Failed to submit event to API: {e}");
        }
    }
}

pub(crate) struct VmWorkerHandle {
    sender: Sender<WorkerCommand>,
    #[allow(dead_code)]
    join_handle: JoinHandle<()>,
}

impl VmWorkerHandle {
    pub(crate) async fn delete_vm(&self) {
        self.send_command(WorkerCommand::DeleteVm).await;
    }

    async fn send_command(&self, command: WorkerCommand) {
        if self.sender.send(command).await.is_err() {
            error!("Worker receiver dropped");
        }
    }
}

#[derive(Debug, EnumDiscriminants)]
enum WorkerCommand {
    DeleteVm,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::{nilcc_api::MockNilccApiClient, qemu::MockVmClient};
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
            display_gtk: false,
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
        vm_client.expect_start_vm().with(eq(spec.clone()), eq(socket.clone())).once().return_once(move |_, _| Ok(()));
        vm_client.expect_stop_vm().with(eq(socket.clone()), eq(true)).once().return_once(move |_, _| Ok(()));

        nilcc_api_client
            .expect_report_vm_event()
            .with(eq(id), eq(VmEvent::Started))
            .once()
            .return_once(move |_, _| Ok(()));
        nilcc_api_client
            .expect_report_vm_event()
            .with(eq(id), eq(VmEvent::Stopped))
            .once()
            .return_once(move |_, _| Ok(()));

        let join_handle = {
            let handle = VmWorker::spawn(id, Arc::new(vm_client), Arc::new(nilcc_api_client), spec, socket);
            handle.delete_vm().await;
            handle.join_handle
        };
        join_handle.await.expect("failed to join");
    }
}
