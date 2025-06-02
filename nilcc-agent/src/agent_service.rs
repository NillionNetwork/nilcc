use crate::grpc::nilcc::agent::v1::{
    info::{AgentInfo, AllocatableResources, MachineInfo, VirtualizationInfo},
    registration::{
        registration_service_client::RegistrationServiceClient, RegisterAgentRequest, RegisterAgentResponse,
    },
};
use anyhow::bail;
use tonic::{transport::Channel, Request};
use tracing::debug;
use uuid::Uuid;

pub struct AgentService {
    grpc_client: RegistrationServiceClient<Channel>,
    endpoint_address: String,
}

impl AgentService {
    pub async fn connect(endpoint_address: String) -> anyhow::Result<Self> {
        debug!("Attempting to connect AgentService to endpoint: {}", endpoint_address);

        let client = RegistrationServiceClient::connect(endpoint_address.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to gRPC endpoint {}: {}", endpoint_address, e))?;

        debug!("Successfully connected AgentService to {}", endpoint_address);

        Ok(Self { grpc_client: client, endpoint_address })
    }

    pub async fn register(
        &mut self,
        agent_info: AgentInfo,
        machine_info: MachineInfo,
        virtualization_info: VirtualizationInfo,
        allocatable_resources: AllocatableResources,
    ) -> anyhow::Result<RegisterAgentResponse> {
        debug!("Preparing to register agent_id: {} with version: {}", agent_info.agent_id, agent_info.agent_version);

        let request_payload = RegisterAgentRequest {
            agent_info: Some(agent_info),
            machine_info: Some(machine_info),
            virtualization_info: Some(virtualization_info),
            allocatable_resources: Some(allocatable_resources),
        };

        let request = Request::new(request_payload);

        debug!("Sending RegisterAgentRequest to {}", self.endpoint_address);

        match self.grpc_client.register_agent(request).await {
            Ok(response) => {
                let inner_response = response.into_inner();
                if inner_response.success {
                    debug!("Agent registered successfully with server. Server message: {}", inner_response.message);
                } else {
                    debug!(
                        "Agent registration reported as not successful by server. Server message: {}",
                        inner_response.message
                    );
                }
                Ok(inner_response)
            }
            Err(status) => {
                bail!("gRPC error during agent registration: code={}, message='{}'", status.code(), status.message())
            }
        }
    }

    pub async fn report_status(&mut self, agent_id: Uuid) -> anyhow::Result<()> {
        debug!("Reporting status for agent_id: {}", agent_id);

        //TODO: Send AgentCondition message to the server

        Ok(())
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint_address
    }
}
