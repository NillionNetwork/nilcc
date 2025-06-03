use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MetalInstanceDetails {
    pub id: Uuid,
    pub agent_version: String,
    pub hostname: String,
    pub memory: u64,
    pub disk: u64,
    pub cpu: u32,
    pub gpu: u32,
    pub gpu_model: Option<String>,
    pub ip_address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegistrationResponse {
    pub agent_id: String,
    pub message: String,
    pub success: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncRequest {
    /// If an agent is available to perform tasks
    pub available: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncResponse {
    pub success: bool,
    pub message: String,
}
