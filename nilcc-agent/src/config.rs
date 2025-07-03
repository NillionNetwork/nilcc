use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use std::{net::SocketAddr, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub struct QemuConfig {
    /// Path to the qemu-system binary
    pub system_bin: PathBuf,

    /// Path to the qemu-img binary
    pub img_bin: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ControllerConfig {
    /// nilcc API endpoint to connect to.
    pub endpoint: String,

    /// nilcc API key.
    pub key: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiConfig {
    /// The endpoint to bind to.
    pub bind_endpoint: SocketAddr,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DbConfig {
    /// The database URL.
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SniProxyConfigTimeouts {
    /// Timeout for connection establishment in ms.
    pub connect: u64,

    /// Timeout for server in ms.
    pub server: u64,

    /// Timeout for client in ms.
    pub client: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MetricsConfig {
    /// The endpoint where metrics are exposed.
    pub bind_endpoint: SocketAddr,
}

/// The resources configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct ResourcesConfig {
    /// The reserved resources.
    pub reserved: ReservedResourcesConfig,

    /// The resource limits for VMs.
    #[serde(default)]
    pub limits: ResourceLimitsConfig,
}

/// The reserved resources configuration.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReservedResourcesConfig {
    /// The number of reserved CPUs.
    pub cpus: u32,

    /// The reserved memory in MBs.
    pub memory_mb: u32,

    /// The reserved disk space in GBs.
    pub disk_space_gb: u32,
}

/// The resource limits configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct ResourceLimitsConfig {
    /// The maximum number of CPUs.
    #[serde(default = "u32_max")]
    pub cpus: u32,

    /// The maximum memory in MBs.
    #[serde(default = "u32_max")]
    pub memory_mb: u32,

    /// The maximum disk space in GBs.
    #[serde(default = "u32_max")]
    pub disk_space_gb: u32,
}

impl Default for ResourceLimitsConfig {
    fn default() -> Self {
        Self { cpus: u32::MAX, memory_mb: u32::MAX, disk_space_gb: u32::MAX }
    }
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

fn u32_max() -> u32 {
    u32::MAX
}
