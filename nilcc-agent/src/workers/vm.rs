use crate::clients::qemu::{VmClient, VmSpec};
use std::{path::PathBuf, sync::Arc};
use strum::EnumDiscriminants;
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task::JoinHandle,
};
use tracing::{error, info, info_span, Instrument};
use uuid::Uuid;

pub(crate) struct VmWorker {
    vm_client: Arc<dyn VmClient>,
    spec: VmSpec,
    socket_path: PathBuf,
    receiver: Receiver<WorkerCommand>,
}

impl VmWorker {
    pub(crate) fn spawn(
        vm_id: Uuid,
        vm_client: Arc<dyn VmClient>,
        spec: VmSpec,
        socket_path: PathBuf,
    ) -> VmWorkerHandle {
        let (sender, receiver) = channel(64);
        let join_handle = tokio::spawn(async move {
            let worker = VmWorker { vm_client, spec, socket_path, receiver };
            worker.run().instrument(info_span!("vm_worker", vm_id = vm_id.to_string())).await;
        });
        VmWorkerHandle { sender, join_handle }
    }

    async fn run(mut self) {
        if let Err(e) = self.vm_client.start_vm(self.spec.clone(), &self.socket_path).await {
            // TODO: do something
            error!("Failed to start VM: {e}");
            return;
        }

        while let Some(command) = self.receiver.recv().await {
            let discriminant = WorkerCommandDiscriminants::from(&command);
            info!("Received {discriminant:?} command");
            if let Err(e) = self.handle_command(command).await {
                error!("Failed to process {discriminant:?} command: {e}")
            }
        }
        info!("Exiting run loop");
    }

    async fn handle_command(&self, command: WorkerCommand) -> anyhow::Result<()> {
        match command {
            WorkerCommand::StopVm => Ok(self.vm_client.stop_vm(&self.socket_path, true).await?),
        }
    }
}

pub(crate) struct VmWorkerHandle {
    sender: Sender<WorkerCommand>,
    #[allow(dead_code)]
    join_handle: JoinHandle<()>,
}

impl VmWorkerHandle {
    pub(crate) async fn stop_vm(&self) {
        self.send_command(WorkerCommand::StopVm).await;
    }

    async fn send_command(&self, command: WorkerCommand) {
        if self.sender.send(command).await.is_err() {
            error!("Worker receiver dropped");
        }
    }
}

#[derive(Debug, EnumDiscriminants)]
enum WorkerCommand {
    StopVm,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::qemu::MockVmClient;
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
        let mut client = MockVmClient::default();
        client.expect_start_vm().with(eq(spec.clone()), eq(socket.clone())).once().return_once(move |_, _| Ok(()));
        client.expect_stop_vm().with(eq(socket.clone()), eq(true)).once().return_once(move |_, _| Ok(()));

        let join_handle = {
            let handle = VmWorker::spawn(id, Arc::new(client), spec, socket);
            handle.stop_vm().await;
            handle.join_handle
        };
        join_handle.await.expect("failed to join");
    }
}
