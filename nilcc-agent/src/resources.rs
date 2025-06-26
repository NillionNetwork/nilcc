use crate::config::ReservedResourcesConfig;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};
use tokio::process::Command;
use tracing::info;

const SUPPORTED_GPU_MODEL: &str = "H100";
const NVIDIA_GPU_VENDOR_ID: &str = "10de";

#[derive(Debug, Clone, Serialize)]
pub struct SystemResources {
    pub(crate) hostname: String,
    pub(crate) memory_gb: u64,
    pub(crate) reserved_memory_gb: u64,
    pub(crate) disk_space_gb: u64,
    pub(crate) reserved_disk_space_gb: u64,
    pub(crate) cpus: u32,
    pub(crate) reserved_cpus: u32,
    pub(crate) gpus: Option<Gpus>,
}

impl SystemResources {
    // Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
    pub async fn gather(reserved: ReservedResourcesConfig) -> anyhow::Result<Self> {
        info!("Gathering metal instance details");

        let sys = System::new_all();

        let hostname = System::host_name().context("Failed to get hostname from sysinfo")?;

        let memory_gb = sys.total_memory() / (1024 * 1024 * 1024);
        if reserved.memory_gb > memory_gb {
            bail!("Reserved memory ({}) exceeds total memory ({memory_gb})", reserved.memory_gb);
        }

        let disks = Disks::new_with_refreshed_list();
        let mut root_disk_bytes = 0;
        for disk in disks.list() {
            if disk.mount_point().as_os_str() == "/" {
                root_disk_bytes = disk.total_space();
            }
        }
        let disk_size_gb = root_disk_bytes / (1024 * 1024 * 1024);
        if reserved.disk_space_gb > disk_size_gb {
            bail!("Reserved disk space ({}) exceeds total disk space ({disk_size_gb})", reserved.disk_space_gb);
        }

        let cpus = sys.cpus().len() as u32;
        if reserved.cpus > cpus {
            bail!("Reserved CPUs ({}) exceed total CPUs ({cpus})", reserved.cpus);
        }

        let gpus = Self::find_gpus().await?;
        Ok(Self {
            hostname,
            memory_gb,
            reserved_memory_gb: reserved.memory_gb,
            disk_space_gb: disk_size_gb,
            reserved_disk_space_gb: reserved.disk_space_gb,
            cpus,
            reserved_cpus: reserved.cpus,
            gpus,
        })
    }

    /// Finds supported NVIDIA GPUs
    pub(crate) async fn find_gpus() -> anyhow::Result<Option<Gpus>> {
        let output = Command::new("bash").arg("-c").arg(format!("lspci -d {NVIDIA_GPU_VENDOR_ID}:")).output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("lspci command failed with status {}: {stderr}", output.status.code().unwrap_or_default());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().filter(|&line| !line.trim().is_empty()).collect();
        if lines.is_empty() {
            return Ok(None);
        }

        let mut addresses = Vec::new();
        for line in lines {
            if line.contains("H100") {
                if let Some(bdf) = line.split_whitespace().next() {
                    addresses.push(GpuAddress(bdf.to_string()));
                } else {
                    bail!(format!("Failed to parse BDF address from line: {line}"));
                }
            } else {
                bail!(format!(
                    "Unsupported NVIDIA GPU found. All GPUs must be {SUPPORTED_GPU_MODEL}. Detected: {line}"
                ));
            }
        }

        addresses.sort();

        Ok(Some(Gpus { model: SUPPORTED_GPU_MODEL.to_string(), addresses }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct GpuAddress(pub(crate) String);

impl From<&'_ str> for GpuAddress {
    fn from(address: &str) -> Self {
        Self(address.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Gpus {
    pub(crate) model: String,
    pub(crate) addresses: Vec<GpuAddress>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn gather() {
        let resources = SystemResources::gather(Default::default()).await.expect("failed to gather resources");
        assert!(resources.cpus > 0);
        assert!(resources.disk_space_gb > 0);
    }

    #[tokio::test]
    async fn gather_too_much_reserved_cpu() {
        let reserved = ReservedResourcesConfig { cpus: 1024, memory_gb: 0, disk_space_gb: 0 };
        SystemResources::gather(reserved).await.expect_err("gathering did not fail");
    }

    #[tokio::test]
    async fn gather_too_much_reserved_memory() {
        let reserved = ReservedResourcesConfig { cpus: 0, memory_gb: 1024, disk_space_gb: 0 };
        SystemResources::gather(reserved).await.expect_err("gathering did not fail");
    }

    #[tokio::test]
    async fn gather_too_much_reserved_disk() {
        let reserved = ReservedResourcesConfig { cpus: 0, memory_gb: 0, disk_space_gb: 100_000 };
        SystemResources::gather(reserved).await.expect_err("gathering did not fail");
    }
}
