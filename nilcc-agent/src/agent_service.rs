use crate::{
    build_info::get_agent_version,
    data_schemas::{MetalInstance, MetalInstanceDetails, SyncRequest},
    gpu,
    http_client::NilccApiClient,
};
use anyhow::{Context, Result};
use std::time::Duration;
use sysinfo::{Disks, System};
use tokio::sync::watch;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Arguments to the agent service.
pub struct AgentServiceArgs {
    /// The identifier for this agent.
    pub agent_id: Uuid,

    /// The nilcc-api client.
    pub api_client: Box<dyn NilccApiClient>,

    /// The interval to use for sync requests.
    pub sync_interval: Duration,
}

pub struct AgentService {
    agent_id: Uuid,
    api_client: Box<dyn NilccApiClient>,
    sync_interval: Duration,
}

impl AgentService {
    pub fn new(args: AgentServiceArgs) -> Self {
        let AgentServiceArgs { agent_id, api_client, sync_interval } = args;
        Self { agent_id, api_client, sync_interval }
    }

    /// Starts the agent service: registers the agent and begins periodic syncing.
    #[tracing::instrument("service.run", skip_all, fields(agent_id = self.agent_id.to_string()))]
    pub async fn run(mut self) -> Result<AgentServiceHandle> {
        info!("Starting run sequence");

        self.perform_registration().await.context("Initial agent registration failed")?;

        let handle = self.spawn_sync_executor();
        info!("AgentService is now operational. Registration complete and status reporter started");

        Ok(handle)
    }

    /// Gathers system information and registers the agent with the API server.
    async fn perform_registration(&mut self) -> Result<()> {
        info!("Attempting to register agent...");

        let details = gather_metal_instance_details().await?;
        let instance = MetalInstance { id: self.agent_id, details };
        info!("Metal instance: {instance:?}");

        self.api_client.register(instance).await.inspect_err(|e| {
            error!("Agent registration failed: {e:#}");
        })?;

        info!("Successfully registered with API");
        Ok(())
    }

    /// Spawns a Tokio task that periodically reports the agent's status.
    fn spawn_sync_executor(self) -> AgentServiceHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        info!("Spawning periodic sync executor");

        tokio::spawn(async move { self.run_loop(shutdown_rx).await });
        AgentServiceHandle(shutdown_tx)
    }

    async fn run_loop(self, mut shutdown_rx: watch::Receiver<()>) {
        let mut interval = tokio::time::interval(self.sync_interval);

        info!("Sync task started. Will report every {:?}", self.sync_interval);

        loop {
            tokio::select! {
                biased; // prefer shutdown signal if available simultaneously
                _ = shutdown_rx.changed() => {
                    info!("Received shutdown signal. Exiting task.");
                    break;
                }
                _ = interval.tick() => {

                    let sync_request = SyncRequest {
                        // TODO: Populate the sync request with necessary data.
                    };

                    match self.api_client.sync(self.agent_id, sync_request).await {
                        Ok(response) => {
                            info!("Successfully synced. Server response: {response:?}");
                        }
                        Err(e) => {
                            //TODO: Consider more robust error handling: e.g., retries with backoff.
                            error!("Failed to sync {e:#}");
                        }
                    }
                }
            }
        }
        info!("Sync executor task for agent has finished.");
    }
}

#[must_use]
pub struct AgentServiceHandle(watch::Sender<()>);

impl Drop for AgentServiceHandle {
    fn drop(&mut self) {
        if self.0.send(()).is_err() {
            warn!("Task might have already exited");
        }
    }
}

// Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
pub async fn gather_metal_instance_details() -> Result<MetalInstanceDetails> {
    info!("Gathering metal instance details...");

    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name().context("Failed to get hostname from sysinfo")?;
    let memory = sys.total_memory() / (1024 * 1024 * 1024);
    let disks = Disks::new_with_refreshed_list();
    let mut root_disk_bytes = 0;
    for disk in disks.list() {
        if disk.mount_point().as_os_str() == "/" {
            root_disk_bytes = disk.total_space();
        }
    }
    let disk = root_disk_bytes / (1024 * 1024 * 1024);
    let cpu = sys.cpus().len() as u32;
    let gpu_group = gpu::find_gpus().await?;

    let (gpu_model, gpu_count) =
        gpu_group.map(|group| (Some(group.model.clone()), Some(group.addresses.len() as u32))).unwrap_or_default();

    let details = MetalInstanceDetails {
        agent_version: get_agent_version().to_string(),
        hostname,
        memory,
        disk,
        cpu,
        gpu: gpu_count,
        gpu_model,
    };

    Ok(details)
}
