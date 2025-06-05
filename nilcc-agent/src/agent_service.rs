use crate::{
    build_info::get_agent_version,
    config::GpuDetails,
    data_schemas::{MetalInstanceDetails, SyncRequest},
    http_client::AgentHttpRestClient,
};
use anyhow::{Context, Result};
use std::{net::IpAddr, sync::Arc, time::Duration};
use sysinfo::{Disks, IpNetwork, NetworkData, Networks, System};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const DEFAULT_AGENT_SYNC_INTERVAL: Duration = Duration::from_secs(10);

pub struct AgentServiceBuilder {
    agent_id: Uuid,
    nilcc_api_base_url: String,
    api_key: Option<String>,
    gpu: Option<GpuDetails>,
    sync_interval: Option<Duration>,
}

impl AgentServiceBuilder {
    /// Creates a new builder.
    fn new(agent_id: Uuid, nilcc_api_base_url: String) -> Self {
        Self { agent_id, nilcc_api_base_url, api_key: None, sync_interval: None, gpu: None }
    }

    /// Sets the optional API key for authenticating requests to the nilCC API.
    pub fn api_key(mut self, key: String) -> Self {
        self.api_key = Some(key);
        self
    }

    /// Sets the interval for the periodic synchronization (status reporting) task, defaults to DEFAULT_AGENT_SYNC_INTERVAL (e.g. 10 seconds).
    pub fn sync_interval(mut self, interval: Duration) -> Self {
        self.sync_interval = Some(interval);
        self
    }

    /// Sets the GPU details of the agents machine.
    pub fn gpu(mut self, gpu: GpuDetails) -> Self {
        self.gpu = Some(gpu);
        self
    }

    /// Consumes the builder and constructs the AgentService instance.
    pub fn build(self) -> Result<AgentService> {
        let sync_interval = self.sync_interval.unwrap_or(DEFAULT_AGENT_SYNC_INTERVAL);
        let http_client = Arc::new(AgentHttpRestClient::new(self.nilcc_api_base_url.clone(), self.api_key)?);

        info!("Agent ID: {}", self.agent_id);
        info!("nilCC API: {}", self.nilcc_api_base_url);

        Ok(AgentService { http_client, agent_id: self.agent_id, sync_interval, gpu: self.gpu, sync_executor: None })
    }
}

pub struct AgentService {
    http_client: Arc<AgentHttpRestClient>,
    agent_id: Uuid,
    sync_interval: Duration,
    gpu: Option<GpuDetails>,
    sync_executor: Option<watch::Sender<()>>,
}

impl AgentService {
    /// Returns a new builder for `AgentService`.
    pub fn builder(agent_id: Uuid, nilcc_api_base_url: String) -> AgentServiceBuilder {
        AgentServiceBuilder::new(agent_id, nilcc_api_base_url)
    }

    /// Starts the agent service: registers the agent and begins periodic syncing.
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting run sequence");

        self.perform_registration().await.context("Initial agent registration failed")?;
        self.spawn_sync_executor();

        info!(
            "AgentService for {} is now operational. Registration complete and status reporter started.",
            self.agent_id
        );

        Ok(())
    }

    /// Gathers system information and registers the agent with the API server.
    async fn perform_registration(&mut self) -> Result<()> {
        info!("Attempting to register agent...");

        let instance_details = gather_metal_instance_details(self.agent_id, self.gpu.clone())?;
        info!("Metal instance details: {instance_details:?}");

        let response = self.http_client.register(instance_details).await.map_err(|e| {
            error!("Agent registration failed: {}", e);
            e
        })?;

        info!("Agent {} successfully registered with API. Server message: {response:?}", self.agent_id);
        Ok(())
    }

    /// Spawns a Tokio task that periodically reports the agent's status.
    fn spawn_sync_executor(&mut self) {
        if self.sync_executor.is_some() {
            warn!("Sync executor task already spawned for agent_id: {}. Skipping.", self.agent_id);
            return;
        }

        let client = Arc::clone(&self.http_client);
        let agent_id = self.agent_id;
        let sync_interval = self.sync_interval;

        let (shutdown_tx, mut shutdown_rx) = watch::channel(());
        self.sync_executor = Some(shutdown_tx);

        info!("Spawning periodic sync executor for agent_id: {agent_id}");

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(sync_interval);

            info!("Sync task started. Will report every {sync_interval:?}.");

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

                        match client.sync(agent_id, sync_request).await {
                            Ok(response) => {
                                info!("Successfully synced agent {agent_id}. Server response: {response:?}");
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
        });
    }

    /// Requests the shutdown of the periodic sync executor task.
    pub fn request_shutdown(&mut self) {
        if let Some(tx) = self.sync_executor.take() {
            info!("Sending shutdown signal");
            if tx.send(()).is_err() {
                warn!("Task might have already exited.");
            }
        } else {
            warn!("Shutdown already requested or sync task not started");
        }
    }
}

impl Drop for AgentService {
    fn drop(&mut self) {
        info!("AgentService for agent_id {} is being dropped. Requesting shutdown of sync executor.", self.agent_id);
        self.request_shutdown();
    }
}

// Finds a suitable IPv4 address (prefers public, or private if public not found) from the system's network interfaces.
fn find_ipv4_address(networks: Networks) -> Result<String> {
    let mut sorted_interfaces: Vec<(&String, &NetworkData)> = networks.iter().collect();
    sorted_interfaces.sort_unstable_by_key(|(name, _)| *name);

    let mut private_ipv4: String = String::new(); // Fallback for private IPv4 if no public address is found

    for (interface_name, interface) in sorted_interfaces {
        debug!("Inspecting network interface: '{interface_name}' (MAC: {})", interface.mac_address());

        let mut sorted_ip_networks: Vec<&IpNetwork> = interface.ip_networks().iter().collect();
        sorted_ip_networks.sort_unstable_by_key(|ip_net| ip_net.addr);

        for ip_network in sorted_ip_networks {
            if let IpAddr::V4(ipv4_addr) = ip_network.addr {
                if ipv4_addr.is_loopback() || ipv4_addr.is_unspecified() {
                    debug!(
                        "  Skipping loopback or unspecified IPv4 address: {} on interface '{}'.",
                        ipv4_addr, interface_name
                    );
                    continue;
                }

                // Check if the IPv4 address is globally routable (public)
                if ipv4_addr.is_global() {
                    let public_ip = ipv4_addr.to_string();
                    debug!("  Found public (globally routable) IPv4: {public_ip} on interface '{interface_name}'.",);
                    return Ok(public_ip);
                }

                // If not public, check if it's private
                if ipv4_addr.is_private() {
                    let private_ip = ipv4_addr.to_string();
                    debug!(
                        "  Found suitable private IPv4: {private_ip} on interface '{interface_name}'. Storing as fallback.",
                    );
                    private_ipv4 = private_ip;
                } else {
                    debug!(
                        "  IPv4 {} is not global and not private (e.g., link-local, other special). Skipping for fallback consideration.",
                        ipv4_addr
                    );
                }
            }
        }
    }

    if !private_ipv4.is_empty() {
        warn!("No public IPv4 address found. Returning last suitable private IPv4: {}", private_ipv4);
    } else {
        return Err(anyhow::anyhow!("No suitable IPv4 address found."));
    }

    Ok(private_ipv4)
}

// Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
fn gather_metal_instance_details(agent_id: Uuid, gpu_details: Option<GpuDetails>) -> Result<MetalInstanceDetails> {
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
    let (gpu_model, gpu) = if let Some(gpu_details) = gpu_details {
        (Some(gpu_details.model.clone()), Some(gpu_details.count))
    } else {
        (None, None)
    };

    let ip_address =
        find_ipv4_address(Networks::new_with_refreshed_list()).context("Failed to find a suitable IPv4 address")?;

    let details = MetalInstanceDetails {
        id: agent_id,
        agent_version: get_agent_version().to_string(),
        hostname,
        memory,
        disk,
        cpu,
        gpu,
        gpu_model,
        ip_address,
    };

    Ok(details)
}
