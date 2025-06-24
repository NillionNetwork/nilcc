use crate::config::ReservedResourcesConfig;
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt, io};
use sysinfo::{Disks, System};
use tokio::{fs, process::Command};
use tracing::{info, warn};

const H100_MODEL: &str = "H100";
const NVIDIA_GPU_VENDOR_ID: &str = "10de";
const NEW_VFIO_PCI_ID_PATH: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";

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

    pub async fn create_gpu_vfio_devices(&self) -> anyhow::Result<()> {
        let Some(gpus) = &self.gpus else {
            return Ok(());
        };
        info!("Creating PCI VFIO devices for {} GPUs", gpus.addresses.len());
        for address in &gpus.addresses {
            info!("Finding device id for {address}");
            let output = Command::new("lspci").arg("-n").arg("-s").arg(&address.0).invoke().await?;
            let device_id = Self::parse_device_id(&output).context("Failed to parse device id")?;
            info!("Creating PCI VFIO for device {device_id}");

            let command = format!("{NVIDIA_GPU_VENDOR_ID} {device_id}");
            match fs::write(NEW_VFIO_PCI_ID_PATH, &command).await {
                Ok(()) => info!("PCI VFIO device {device_id} created"),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    warn!("PCI VFIO device {device_id} already exists, ignoring")
                }
                Err(e) => Err(e).context("Failed to create PCI VFIO device {device_id}")?,
            }
        }
        Ok(())
    }

    /// Finds supported NVIDIA GPUs
    pub(crate) async fn find_gpus() -> anyhow::Result<Option<Gpus>> {
        let output = Command::new("lspci").arg("-d").arg(format!("{NVIDIA_GPU_VENDOR_ID}:")).invoke().await?;
        Self::parse_gpus(&output).context("Failed to parse GPUs")
    }

    fn parse_gpus(lspci_output: &str) -> anyhow::Result<Option<Gpus>> {
        let lines: Vec<&str> = lspci_output.lines().filter(|&line| !line.trim().is_empty()).collect();
        if lines.is_empty() {
            return Ok(None);
        }

        let mut addresses = Vec::new();
        for line in lines {
            if !line.contains("H100") {
                bail!("Unsupported NVIDIA GPU found. All GPUs must be {H100_MODEL}. Detected: {line}");
            }
            if let Some(bdf) = line.split_whitespace().next() {
                addresses.push(GpuAddress(bdf.to_string()));
            } else {
                bail!("Failed to parse BDF address from line: {line}");
            }
        }

        addresses.sort();

        Ok(Some(Gpus { model: H100_MODEL.to_string(), addresses }))
    }

    fn parse_device_id(lspci_output: &str) -> anyhow::Result<String> {
        // 01:00.0 0302: 10de:2331 (rev a1)
        let device = lspci_output
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| anyhow!("Not enough whitespaces in lspci output: {lspci_output}"))?;
        let (_, device_id) =
            device.split_once(':').ok_or_else(|| anyhow!("No colon in lspci output: {lspci_output}"))?;
        Ok(device_id.to_string())
    }
}

#[async_trait]
trait CommandExt {
    async fn invoke(&mut self) -> anyhow::Result<String>;
}

#[async_trait]
impl CommandExt for Command {
    async fn invoke(&mut self) -> anyhow::Result<String> {
        let output = self.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("lspci command failed with status {}: {stderr}", output.status.code().unwrap_or_default());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

impl fmt::Display for GpuAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
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

    #[test]
    fn parse_h100() {
        let input = [
            "01:00.1 3D controller: NVIDIA Corporation GH100 [H100 PCIe] (rev a1)",
            "01:00.0 3D controller: NVIDIA Corporation GH100 [H100 PCIe] (rev a1)",
        ]
        .join("\n");
        let gpus = SystemResources::parse_gpus(&input).expect("failed to parse").expect("no gpus detected");
        assert_eq!(gpus.model, H100_MODEL);
        assert_eq!(gpus.addresses, &["01:00.0".into(), "01:00.1".into()]);
    }

    #[test]
    fn parse_device_id() {
        let input = "01:00.0 0302: 10de:2331 (rev a1)";
        let id = SystemResources::parse_device_id(input).expect("failed to parse");
        assert_eq!(id, "2331");
    }
}
