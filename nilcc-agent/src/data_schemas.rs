use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    num::NonZeroU16,
};
use strum::{Display, EnumString};
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
pub struct SyncRequest {
    pub id: Uuid,
    pub workloads: Vec<SyncWorkload>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncWorkload {
    pub id: Uuid,
    pub status: WorkloadStatus,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    pub workloads: Vec<Workload>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
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
    pub(crate) status: WorkloadStatus,
}

impl fmt::Debug for Workload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            id,
            docker_compose,
            env_vars,
            service_to_expose,
            service_port_to_expose,
            memory,
            cpu,
            disk,
            gpu,
            status,
        } = self;
        // Hide this one since it can have sensitive data
        let env_vars: BTreeMap<_, _> = env_vars.keys().map(|key| (key, "...")).collect();
        f.debug_struct("Workload")
            .field("id", id)
            .field("docker_compose", docker_compose)
            .field("env_vars", &env_vars)
            .field("service_to_expose", service_to_expose)
            .field("service_port_to_expose", service_port_to_expose)
            .field("memory", memory)
            .field("cpu", cpu)
            .field("disk", disk)
            .field("gpu", gpu)
            .field("status", status)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, EnumString, Display)]
#[serde(rename_all = "camelCase")]
pub enum WorkloadStatus {
    #[default]
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}
