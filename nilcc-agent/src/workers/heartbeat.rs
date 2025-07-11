use crate::clients::nilcc_api::NilccApiClient;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{debug, warn};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

pub struct HeartbeatWorker {
    client: Arc<dyn NilccApiClient>,
}

impl HeartbeatWorker {
    pub fn spawn(client: Arc<dyn NilccApiClient>) {
        tokio::spawn(async move {
            let worker = Self { client };
            worker.run().await
        });
    }

    async fn run(self) {
        loop {
            debug!("Sending heartbeat");
            if let Err(e) = self.client.heartbeat().await {
                warn!("Could not submit heartbeat: {e}");
            }
            debug!("Sleeping for {HEARTBEAT_INTERVAL:?}");
            sleep(HEARTBEAT_INTERVAL).await;
        }
    }
}
