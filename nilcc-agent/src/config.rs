use serde::Deserialize;
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct AgentConfig {
    /// Directory where VM folders live
    pub vm_store: PathBuf,

    /// Confidential VM configuration.
    pub cvm: CvmConfig,

    /// Qemu configuration.
    pub qemu: QemuConfig,

    /// Unique agent ID, used to identify this agent in nilCC server
    pub agent_id: Uuid,

    /// nilCC API configuration.
    pub api: ApiConfig,

    /// The database configuration.
    pub db: DbConfig,
}

#[derive(Deserialize, Debug)]
pub struct CvmConfig {
    /// The path to the initrd file.
    pub initrd: PathBuf,

    /// The path to the kernel file.
    pub kernel: PathBuf,

    /// The path to the bios file.
    pub bios: PathBuf,

    /// The path to the base disk.
    pub base_disk: PathBuf,

    /// The path to the verity disk.
    pub verity_disk: PathBuf,

    /// The verity root hash.
    pub verity_root_hash: String,
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

#[derive(Deserialize, Debug)]
pub struct DbConfig {
    /// The database URL.
    pub url: String,
}

fn default_agent_sync_interval() -> Duration {
    Duration::from_secs(10)
}
