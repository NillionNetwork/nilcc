use crate::clients::qemu::{VmClient, VmSpec};
use std::{path::PathBuf, sync::Arc};
use tracing::error;

pub(crate) struct VmWorker {
    vm_client: Arc<dyn VmClient>,
    spec: VmSpec,
    socket_path: PathBuf,
}

impl VmWorker {
    pub(crate) fn spawn(vm_client: Arc<dyn VmClient>, spec: VmSpec, socket_path: PathBuf) -> VmWorkerHandle {
        tokio::spawn(async move {
            let worker = VmWorker { vm_client, spec, socket_path };
            worker.run().await;
        });
        VmWorkerHandle {}
    }

    async fn run(self) {
        // TODO: do something
        if let Err(e) = self.vm_client.start_vm(self.spec.clone(), &self.socket_path).await {
            error!("Failed to start VM: {e}")
        }
    }
}

pub(crate) struct VmWorkerHandle {}
