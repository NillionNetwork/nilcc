use crate::clients::nilcc_api::{NilccApiClient, NilccApiError, VmEvent};
use reqwest::StatusCode;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    time::sleep,
};
use tracing::{error, info, warn};
use uuid::Uuid;

const RETRY_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct WorkloadEvent {
    workload_id: Uuid,
    event: VmEvent,
}

pub struct EventWorker {
    client: Arc<dyn NilccApiClient>,
    receiver: Receiver<WorkloadEvent>,
}

impl EventWorker {
    pub fn spawn(client: Arc<dyn NilccApiClient>) -> EventSender {
        let (sender, receiver) = channel(1024);
        tokio::spawn(async move {
            let worker = EventWorker { client, receiver };
            worker.run().await;
        });
        EventSender(sender)
    }

    async fn run(mut self) {
        while let Some(event) = self.receiver.recv().await {
            self.send_event(event).await;
        }
    }

    async fn send_event(&self, event: WorkloadEvent) {
        let WorkloadEvent { workload_id, event } = event;
        loop {
            info!("Sending event for workload {workload_id}");
            match self.client.report_vm_event(workload_id, event.clone()).await {
                Ok(_) => return,
                Err(NilccApiError::Api { status, .. }) if status == StatusCode::NOT_FOUND => {
                    warn!("API returned 404 for workload {workload_id} event, ignoring");
                    return;
                }
                Err(e) => {
                    error!("Failed to report event for workload {workload_id}: {e:#}");
                    sleep(RETRY_INTERVAL).await;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct EventSender(pub(crate) Sender<WorkloadEvent>);

impl EventSender {
    pub(crate) async fn send_event(&self, workload_id: Uuid, event: VmEvent) {
        if self.0.send(WorkloadEvent { workload_id, event }).await.is_err() {
            error!("Sender dropped");
        }
    }
}
