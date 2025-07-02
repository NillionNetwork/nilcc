use crate::{
    clients::qemu::{VmClient, VmSpec},
    workers::vm::{VmWorker, VmWorkerHandle},
};
use anyhow::bail;
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use strum::EnumDiscriminants;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{error, info, warn};
use uuid::Uuid;

const CHANNEL_SIZE: usize = 1024;

pub struct VmScheduler {
    receiver: Receiver<SchedulerCommand>,
    vm_client: Arc<dyn VmClient>,
    workers: HashMap<Uuid, VmWorkerHandle>,
}

impl VmScheduler {
    pub fn spawn(qemu_client: Arc<dyn VmClient>) -> Box<dyn VmSchedulerHandle> {
        let (sender, receiver) = channel(CHANNEL_SIZE);
        tokio::spawn(async move {
            let this = VmScheduler { receiver, vm_client: qemu_client, workers: Default::default() };
            this.run().await
        });
        Box::new(DefaultVmSchedulerHandle { sender })
    }

    async fn run(mut self) {
        info!("Starting run loop");
        while let Some(command) = self.receiver.recv().await {
            let discriminant = SchedulerCommandDiscriminants::from(&command);
            info!("Received {discriminant:?} command");
            if let Err(e) = self.handle_command(command).await {
                error!("Failed to process {discriminant:?} command: {e}");
            }
        }
        warn!("Sender dropped, exiting");
    }

    async fn handle_command(&mut self, command: SchedulerCommand) -> anyhow::Result<()> {
        match command {
            SchedulerCommand::StartVm { id, spec, socket_path } => self.start_vm(id, spec, socket_path).await,
            SchedulerCommand::StopVm { id } => self.stop_vm(id).await,
        }
    }

    async fn start_vm(&mut self, id: Uuid, spec: VmSpec, socket_path: PathBuf) -> anyhow::Result<()> {
        match self.workers.get(&id) {
            Some(_) => bail!("VM {id} is already being ran"),
            None => {
                let worker = VmWorker::spawn(id, self.vm_client.clone(), spec, socket_path);
                self.workers.insert(id, worker);
                Ok(())
            }
        }
    }

    async fn stop_vm(&mut self, id: Uuid) -> anyhow::Result<()> {
        match self.workers.remove(&id) {
            Some(worker) => {
                worker.stop_vm().await;
                Ok(())
            }
            None => {
                bail!("VM {id} is not being managed by any worker")
            }
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmSchedulerHandle: Send + Sync {
    async fn start_vm(&self, id: Uuid, spec: VmSpec, socket_path: PathBuf);

    async fn stop_vm(&self, id: Uuid);
}

pub(crate) struct DefaultVmSchedulerHandle {
    sender: Sender<SchedulerCommand>,
}

impl DefaultVmSchedulerHandle {
    async fn send_command(&self, command: SchedulerCommand) {
        if self.sender.send(command).await.is_err() {
            error!("Failed to send command: receiver dropped");
        }
    }
}

#[async_trait]
impl VmSchedulerHandle for DefaultVmSchedulerHandle {
    async fn start_vm(&self, id: Uuid, spec: VmSpec, socket_path: PathBuf) {
        let command = SchedulerCommand::StartVm { id, spec, socket_path };
        self.send_command(command).await;
    }

    async fn stop_vm(&self, id: Uuid) {
        let command = SchedulerCommand::StopVm { id };
        self.send_command(command).await;
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, EnumDiscriminants)]
pub(crate) enum SchedulerCommand {
    StartVm { id: Uuid, spec: VmSpec, socket_path: PathBuf },
    StopVm { id: Uuid },
}
