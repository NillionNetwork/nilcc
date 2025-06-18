use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    num::NonZeroU16,
};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetalInstanceDetails {
    /// A string identifying the agent's version.
    pub agent_version: String,

    /// The machine's hostname.
    pub hostname: String,

    /// The amount of memory in GBs.
    #[serde(rename = "memory")]
    pub memory_gb: u64,

    /// The amount of disk space, in GBs.
    #[serde(rename = "disk")]
    pub disk_space_gb: u64,

    /// The number of CPUs
    #[serde(rename = "cpu")]
    pub cpus: u32,

    /// The number of GPUs.
    #[serde(rename = "gpu")]
    pub gpus: Option<u32>,

    /// The GPU model.
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
    pub(crate) id: Uuid,
    pub(crate) workloads: Vec<SyncWorkload>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncWorkload {
    pub(crate) id: Uuid,
    pub(crate) status: SyncWorkloadStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum SyncWorkloadStatus {
    Running,
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

    #[serde(rename = "memory")]
    pub(crate) memory_gb: u32,

    #[serde(rename = "cpu")]
    pub(crate) cpus: NonZeroU16,

    #[serde(rename = "disk")]
    pub(crate) disk_space_gb: NonZeroU16,

    #[serde(rename = "gpu")]
    pub(crate) gpus: u16,
}

impl fmt::Debug for Workload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            id,
            docker_compose,
            env_vars,
            service_to_expose,
            service_port_to_expose,
            memory_gb,
            cpus,
            disk_space_gb,
            gpus,
        } = self;
        // Hide this one since it can have sensitive data
        let env_vars: BTreeMap<_, _> = env_vars.keys().map(|key| (key, "...")).collect();
        f.debug_struct("Workload")
            .field("id", id)
            .field("docker_compose", docker_compose)
            .field("env_vars", &env_vars)
            .field("service_to_expose", service_to_expose)
            .field("service_port_to_expose", service_port_to_expose)
            .field("memory_gb", memory_gb)
            .field("cpus", cpus)
            .field("disk_space_gb", disk_space_gb)
            .field("gpus", gpus)
            .finish()
    }
}
