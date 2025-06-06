use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetalInstanceDetails {
    pub id: Uuid,
    pub agent_version: String,
    pub hostname: String,
    pub memory: u64,
    pub disk: u64,
    pub cpu: u32,
    pub gpu: Option<u32>,
    pub gpu_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmptyResponse {}
