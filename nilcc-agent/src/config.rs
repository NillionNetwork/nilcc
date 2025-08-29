use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use std::{net::SocketAddr, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    /// Directory where VM folders live
    pub vm_store: PathBuf,

    /// Confidential VM configuration.
    pub cvm: CvmConfigs,

    /// Qemu configuration.
    pub qemu: QemuConfig,

    /// Unique agent ID, used to identify this agent in nilcc server
    pub agent_id: Uuid,

    /// nilcc API configuration.
    pub controller: AgentMode,

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

    /// The zero SSL config.
    pub zerossl: ZeroSslConfig,

    /// The docker hub credentials.
    pub docker: DockerConfig,

    /// The optional TLS configuration.
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CvmConfigs {
    // The base path where all configs are.
    pub artifacts_path: PathBuf,

    /// The initial version to use.
    pub initial_version: String,
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
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AgentMode {
    Standalone,

    Remote(ControllerConfig),
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

    /// The public domain where this agent can be reached.
    pub domain: String,

    /// The API key that needs to be presented when making requests to this instance.
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DbConfig {
    /// The database URL.
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SniProxyConfig {
    /// Start of the port range for the SNI proxy.
    pub start_port_range: u16,

    /// The end of the port range for the SNI proxy.
    pub end_port_range: u16,

    /// The path to the HAProxy configuration file.
    pub config_file_path: PathBuf,

    /// The path to the HA proxy master socket.
    #[serde(default = "ha_proxy_master_socket_path")]
    pub master_socket_path: PathBuf,

    /// The timeouts for the SNI proxy.
    #[serde(default)]
    pub timeouts: SniProxyConfigTimeouts,

    /// The maximum number of connections for the SNI proxy.
    pub max_connections: u64,

    /// Whether to tell haproxy to reload the config.
    #[serde(default = "default_true")]
    pub reload_config: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SniProxyConfigTimeouts {
    /// Timeout for connection establishment in ms.
    #[serde(default = "default_connect_timeout")]
    pub connect: u64,

    /// Timeout for server in ms.
    #[serde(default = "default_server_timeout")]
    pub server: u64,

    /// Timeout for client in ms.
    #[serde(default = "default_client_timeout")]
    pub client: u64,
}

impl Default for SniProxyConfigTimeouts {
    fn default() -> Self {
        Self { connect: default_connect_timeout(), server: default_server_timeout(), client: default_client_timeout() }
    }
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

/// Configuration for zero SSL.
#[derive(Clone, Debug, Deserialize)]
pub struct ZeroSslConfig {
    /// The EAB key id.
    pub eab_key_id: String,

    /// The EAB MAC key.
    pub eab_mac_key: String,
}

/// The TLS configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct TlsConfig {
    /// The path to the certificate cache.
    pub cert_cache: PathBuf,

    /// The contact email address to use for ACME requests.
    pub acme_contact: String,
}

/// The docker hub credentials to use.
#[derive(Clone, Debug, Deserialize)]
pub struct DockerConfig {
    /// The username.
    pub username: String,

    /// The password.
    pub password: String,
}

pub fn read_file_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let path = String::deserialize(deserializer)?;
    let output = std::fs::read_to_string(&path)
        .context(format!("Reading verity_root_hash file {path}"))
        .map_err(serde::de::Error::custom)?;
    Ok(output.trim().to_string())
}

fn u32_max() -> u32 {
    u32::MAX
}

fn default_true() -> bool {
    true
}

fn default_connect_timeout() -> u64 {
    5000
}

fn default_client_timeout() -> u64 {
    30000
}

fn default_server_timeout() -> u64 {
    30000
}

fn ha_proxy_master_socket_path() -> PathBuf {
    "/var/run/haproxy-master.sock".into()
}
