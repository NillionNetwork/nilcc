use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use std::{net::SocketAddr, path::PathBuf, time::Duration};
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

    /// nilcc API configuration.
    pub controller: ControllerConfig,

    /// API configuration.
    pub api: ApiConfig,

    /// The database configuration.
    pub db: DbConfig,

    /// The SNI proxy configuration.
    pub sni_proxy: SniProxyConfig,

    /// The metrics configuration.
    pub metrics: MetricsConfig,

    /// The resource configuration.
    pub resources: ResourcesConfig,
}

#[derive(Deserialize, Debug)]
pub struct CvmConfig {
    /// The path to the initrd file.
    pub initrd: PathBuf,

    /// The path to the bios file.
    pub bios: PathBuf,

    /// The disk, kernel and verity files for the cpu cvm.
    pub cpu: CvmFiles,

    /// The disk, kernel and verity files for the gpu cvm.
    pub gpu: CvmFiles,
}

#[derive(Deserialize, Debug)]
pub struct CvmFiles {
    /// The path to the kernel file.
    pub kernel: PathBuf,

    /// The path to the base disk.
    pub base_disk: PathBuf,

    /// The path to the verity disk.
    pub verity_disk: PathBuf,

    /// The path to the verity root hash.
    #[serde(deserialize_with = "read_file_as_string")]
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
pub struct ControllerConfig {
    /// nilcc API endpoint to connect to.
    pub endpoint: String,

    /// nilcc API key.
    pub key: String,

    /// Interval for periodic synchronization task.
    #[serde(with = "humantime_serde", default = "default_agent_sync_interval")]
    pub sync_interval: Duration,
}

#[derive(Deserialize, Debug)]
pub struct ApiConfig {
    /// The endpoint to bind to.
    pub bind_endpoint: SocketAddr,
}

#[derive(Deserialize, Debug)]
pub struct DbConfig {
    /// The database URL.
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct SniProxyConfig {
    /// The DNS subdomain where workloads will be accessible.
    pub dns_subdomain: String,

    /// Start of the port range for the SNI proxy.
    pub start_port_range: u16,

    /// The end of the port range for the SNI proxy.
    pub end_port_range: u16,

    /// The path to the HAProxy configuration file.
    pub config_file_path: String,

    /// The command to reload the HAProxy configuration.
    pub ha_proxy_config_reload_command: String,

    /// The timeouts for the SNI proxy.
    pub timeouts: SniProxyConfigTimeouts,

    /// The maximum number of connections for the SNI proxy.
    pub max_connections: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SniProxyConfigTimeouts {
    /// Timeout for connection establishment in ms.
    pub connect: u64,

    /// Timeout for server in ms.
    pub server: u64,

    /// Timeout for client in ms.
    pub client: u64,
}

#[derive(Deserialize, Debug)]
pub struct MetricsConfig {
    /// The endpoint where metrics are exposed.
    pub bind_endpoint: SocketAddr,
}

/// The resources configuration.
#[derive(Deserialize, Debug)]
pub struct ResourcesConfig {
    /// The reserved resources.
    pub reserved: ReservedResourcesConfig,
}

/// The reserved resources configuration.
#[derive(Deserialize, Debug, Default)]
pub struct ReservedResourcesConfig {
    /// The number of reserved CPUs.
    pub cpus: u32,

    /// The reserved memory in GBs.
    pub memory_gb: u64,

    /// The reserved disk space in GBs.
    pub disk_space_gb: u64,
}

fn default_agent_sync_interval() -> Duration {
    Duration::from_secs(10)
}

pub fn read_file_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let path = String::deserialize(deserializer)?;
    std::fs::read_to_string(&path)
        .context(format!("Reading verity_root_hash file {path}"))
        .map_err(serde::de::Error::custom)
}
