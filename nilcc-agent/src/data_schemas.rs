use serde::{Deserialize, Serialize};
use std::{collections::HashMap, num::NonZeroU16};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetalInstanceDetails {
    pub agent_version: String,
    pub hostname: String,
    pub memory: u64,
    pub disk: u64,
    pub cpu: u32,
    pub gpu: Option<u32>,
    pub gpu_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetalInstance {
    pub id: Uuid,

    #[serde(flatten)]
    pub details: MetalInstanceDetails,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Workload {
    pub(crate) id: Uuid,
    pub(crate) docker_compose: String,
    pub(crate) env_vars: HashMap<String, String>,
    pub(crate) service_to_expose: String,
    pub(crate) service_port_to_expose: u16,
    pub(crate) memory: u32,
    pub(crate) cpu: NonZeroU16,
    pub(crate) disk: NonZeroU16,
    pub(crate) gpu: u16,
}
