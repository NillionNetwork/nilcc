use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod bootstrap {
    use super::*;

    /// The ACME EAB key id environment variable.
    pub const CADDY_ACME_EAB_KEY_ID: &str = "CADDY_ACME_EAB_KEY_ID";

    /// The ACME EAB mac key environment variable.
    pub const CADDY_ACME_EAB_MAC_KEY: &str = "CADDY_ACME_EAB_MAC_KEY";

    /// A request to bootstrap the CVM.
    #[derive(Deserialize, Serialize)]
    pub struct BootstrapRequest {
        /// The ACME EAB key id.
        pub acme_eab_key_id: String,

        /// The ACME EAB MAC key.
        pub acme_eab_mac_key: String,
    }
}

pub mod container {
    use super::*;

    /// A container.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Container {
        /// The names for this container.
        pub names: Vec<String>,

        /// The container image.
        pub image: String,

        /// The container image id.
        pub image_id: String,

        /// The state of this container.
        pub state: String,
    }
}

pub mod health {
    use super::*;

    /// A response to a health request.
    #[derive(Deserialize, Serialize, Validate)]
    #[serde(rename_all = "camelCase")]
    pub struct HealthResponse {
        /// Whether HTTPS is configured and available.
        pub https: bool,

        /// Whether the CVM is bootstrapped
        pub bootstrapped: bool,
    }
}

pub mod logs {
    use super::*;

    /// A request to get the logs for a container.
    #[derive(Deserialize, Serialize, Validate)]
    #[serde(rename_all = "camelCase")]
    pub struct ContainerLogsRequest {
        /// The container that we're pulling logs out of.
        pub container: String,

        /// Whether to pull logs from the tail of the stream.
        pub tail: bool,

        /// The stream to take logs out of.
        pub stream: OutputStream,

        /// The maximum number of log lines to be returned.
        #[validate(range(max = 1000))]
        pub max_lines: usize,
    }

    /// The stream to take logs out of.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum OutputStream {
        /// Standard output.
        Stdout,

        /// Standard error.
        Stderr,
    }

    /// The container logs response.
    #[derive(Deserialize, Serialize)]
    pub struct ContainerLogsResponse {
        /// The log lines.
        pub lines: Vec<String>,
    }

    /// A request to get the system logs.
    #[derive(Deserialize, Serialize, Validate)]
    #[serde(rename_all = "camelCase")]
    pub struct SystemLogsRequest {
        /// The log source to fetch.
        pub source: SystemLogsSource,

        /// Whether to pull logs from the tail of the stream.
        pub tail: bool,

        /// The maximum number of log lines to be returned.
        #[validate(range(max = 1000))]
        pub max_lines: usize,
    }

    /// The source for system logs.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum SystemLogsSource {
        /// Get the cvm-agent logs.
        CvmAgent,
    }

    /// The system logs response.
    #[derive(Deserialize, Serialize)]
    pub struct SystemLogsResponse {
        /// The log lines.
        pub lines: Vec<String>,
    }
}

pub mod stats {
    use std::path::PathBuf;

    use super::*;

    /// The stats response.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct SystemStatsResponse {
        /// Stats about the memory usage.
        pub memory: MemoryStats,

        /// Stats about every CPU.
        pub cpus: Vec<CpuStats>,

        /// Stats about every disk.
        pub disks: Vec<DiskStats>,
    }

    /// Memory stats.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct MemoryStats {
        /// The total memory in the CVM.
        pub total: u64,

        /// The total used memory.
        pub used: u64,
    }

    /// CPU stats.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct CpuStats {
        /// The CPU name.
        pub name: String,

        /// The CPU usage, as a percentage between 0-100.
        pub usage: f32,

        /// The frequency, in MHz.
        pub frequency: u64,
    }

    /// Disk stats.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct DiskStats {
        /// The name of this disk.
        pub name: String,

        /// The path where the filesystem is mounted.
        pub mount_point: PathBuf,

        /// The type of filesystem.
        pub filesystem: String,

        /// The total size of this disk.
        pub size: u64,

        /// The used space this disk.
        pub used: u64,
    }
}
