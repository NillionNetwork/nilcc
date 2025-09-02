use crate::{
    clients::nilcc_api::{NilccApiClient, NilccApiError, VmEvent, VmEventDiscriminants},
    repositories::{sqlite::RepositoryProvider, workload::WorkloadRepositoryError},
};
use anyhow::Context;
use reqwest::StatusCode;
use std::{collections::HashSet, sync::Arc, time::Duration};
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

pub struct EventWorkerArgs {
    pub api_client: Arc<dyn NilccApiClient>,
    pub repository_provider: Arc<dyn RepositoryProvider>,
}

pub struct EventWorker {
    client: Arc<dyn NilccApiClient>,
    receiver: Receiver<WorkloadEvent>,
    repository_provider: Arc<dyn RepositoryProvider>,
    seen_workloads: HashSet<Uuid>,
}

impl EventWorker {
    pub fn spawn(args: EventWorkerArgs) -> EventSender {
        let EventWorkerArgs { api_client, repository_provider } = args;
        let (sender, receiver) = channel(1024);
        tokio::spawn(async move {
            let worker =
                EventWorker { client: api_client, repository_provider, receiver, seen_workloads: Default::default() };
            worker.run().await;
        });
        EventSender(sender)
    }

    async fn run(mut self) {
        while let Some(event) = self.receiver.recv().await {
            while let Err(e) = self.send_event(&event).await {
                error!("Failed to send event: {e:#}");
                sleep(RETRY_INTERVAL).await;
            }
        }
    }

    async fn send_event(&mut self, event: &WorkloadEvent) -> anyhow::Result<()> {
        let WorkloadEvent { workload_id, event } = event;
        let event_type = format!("{:?}", VmEventDiscriminants::from(event));
        let mut repo =
            self.repository_provider.workloads(Default::default()).await.context("Failed to get repository")?;
        if !self.seen_workloads.contains(workload_id) {
            let workload = match repo.find(*workload_id).await {
                Ok(workload) => workload,
                Err(WorkloadRepositoryError::WorkloadNotFound) => {
                    warn!("Ignoring event {event_type} since workload {workload_id} does not exist anymore");
                    return Ok(());
                }
                Err(e) => {
                    return Err(e).context("Failed to lookup workload");
                }
            };
            if workload.last_reported_event.as_ref() == Some(&event_type) {
                info!("Already reported event {event_type} for workload {workload_id}, ignoring");
                self.seen_workloads.insert(*workload_id);
                return Ok(());
            }
        }

        info!("Sending event {event_type} for workload {workload_id}");
        match self.client.report_vm_event(*workload_id, event.clone()).await {
            Ok(_) => (),
            Err(NilccApiError::Api { status, .. }) if status == StatusCode::NOT_FOUND => {
                warn!("API returned 404 for workload {workload_id} event, ignoring");
                return Ok(());
            }
            Err(e) => {
                return Err(e).context("Failed to send event to API");
            }
        };

        loop {
            match repo.set_last_reported_event(*workload_id, event_type.clone()).await {
                Ok(_) => {
                    self.seen_workloads.insert(*workload_id);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Failed to update workload last reported event: {e}");
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
