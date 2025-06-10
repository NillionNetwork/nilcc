use serde::Deserialize;
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct AgentConfig {
    /// Directory where VM folders live
    pub vm_store: PathBuf,

    /// Qemu configuration.
    pub qemu: QemuConfig,

    /// Unique agent ID, used to identify this agent in nilCC server
    pub agent_id: Uuid,

    /// nilCC API configuration.
    pub api: ApiConfig,
}

#[derive(Deserialize, Debug)]
pub struct QemuConfig {
    /// Path to the qemu-system binary
    pub system_bin: PathBuf,

    /// Path to the qemu-img binary
    pub img_bin: PathBuf,
}

#[derive(Deserialize, Debug)]
pub struct ApiConfig {
    /// nilCC API endpoint to connect to.
    pub endpoint: String,

    /// nilCC API key.
    pub key: String,

    /// Interval for periodic synchronization task.
    #[serde(with = "humantime_serde", default = "default_agent_sync_interval")]
    pub sync_interval: Duration,
}

fn default_agent_sync_interval() -> Duration {
    Duration::from_secs(10)
}
