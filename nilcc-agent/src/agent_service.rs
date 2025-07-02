use crate::{
    clients::nilcc_api::NilccApiClient,
    models::{MetalInstance, MetalInstanceDetails},
    resources::SystemResources,
    version::agent_version,
};
use anyhow::{Context, Result};
use tracing::{error, info};
use uuid::Uuid;

/// Arguments to the agent service.
pub struct AgentServiceArgs {
    /// The identifier for this agent.
    pub agent_id: Uuid,

    /// The nilcc-api client.
    pub api_client: Box<dyn NilccApiClient>,

    /// The system resources.
    pub system_resources: SystemResources,
}

pub struct AgentService {
    agent_id: Uuid,
    api_client: Box<dyn NilccApiClient>,
    system_resources: SystemResources,
}

impl AgentService {
    pub fn new(args: AgentServiceArgs) -> Self {
        let AgentServiceArgs { agent_id, api_client, system_resources } = args;
        Self { agent_id, api_client, system_resources }
    }

    /// Starts the agent service: registers the agent and begins periodic syncing.
    #[tracing::instrument("service.run", skip_all, fields(agent_id = self.agent_id.to_string()))]
    pub async fn register(mut self) -> Result<()> {
        info!("Starting run sequence");

        self.perform_registration().await.context("Initial agent registration failed")?;
        info!("AgentService is now operational. Registration complete and status reporter started");

        Ok(())
    }

    /// Gathers system information and registers the agent with the API server.
    async fn perform_registration(&mut self) -> Result<()> {
        info!("Attempting to register agent...");

        let instance = MetalInstance {
            id: self.agent_id,
            details: MetalInstanceDetails {
                agent_version: agent_version().to_string(),
                hostname: self.system_resources.hostname.clone(),
                memory_gb: self.system_resources.memory_gb,
                os_reserved_memory_gb: self.system_resources.reserved_memory_gb,
                disk_space_gb: self.system_resources.disk_space_gb,
                os_reserved_disk_space_gb: self.system_resources.reserved_disk_space_gb,
                cpus: self.system_resources.cpus,
                os_reserved_cpus: self.system_resources.reserved_cpus,
                gpus: self.system_resources.gpus.as_ref().map(|g| g.addresses.len() as u32),
                gpu_model: self.system_resources.gpus.as_ref().map(|g| g.model.clone()),
            },
        };
        info!("Metal instance: {instance:?}");

        self.api_client.register(instance).await.inspect_err(|e| {
            error!("Agent registration failed: {e:#}");
        })?;

        info!("Successfully registered with API");
        Ok(())
    }
}
