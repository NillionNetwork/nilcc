use crate::{
    config::ReservedResourcesConfig,
    repositories::sqlite::{ProviderMode, RepositoryProvider},
};
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt, io,
    net::{IpAddr, Ipv4Addr},
};
use sysinfo::{Disks, Networks, System};
use tokio::{fs, process::Command};
use tracing::{debug, info, warn};

const H100_MODEL: &str = "H100";
const NVIDIA_GPU_VENDOR_ID: &str = "10de";
const NEW_VFIO_PCI_ID_PATH: &str = "/sys/bus/pci/drivers/vfio-pci/new_id";

#[derive(Debug, Clone, Serialize)]
pub struct SystemResources {
    pub hostname: String,
    pub memory_mb: u32,
    pub reserved_memory_mb: u32,
    pub disk_space_gb: u32,
    pub reserved_disk_space_gb: u32,
    pub cpus: u32,
    pub reserved_cpus: u32,
    pub gpus: Option<Gpus>,
}

impl SystemResources {
    // Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
    pub async fn gather(reserved: ReservedResourcesConfig) -> anyhow::Result<Self> {
        info!("Gathering metal instance details");

        let sys = System::new_all();
        let hostname = System::host_name().context("Failed to get hostname from sysinfo")?;
        let memory_mb = (sys.total_memory() / (1024 * 1024)).try_into().context("Too much memory")?;
        if reserved.memory_mb > memory_mb {
            bail!("Reserved memory ({}) exceeds total memory ({memory_mb})", reserved.memory_mb);
        }

        let disks = Disks::new_with_refreshed_list();
        let mut root_disk_bytes = 0;
        for disk in disks.list() {
            if disk.mount_point().as_os_str() == "/" {
                root_disk_bytes = disk.total_space();
            }
        }
        let disk_space_gb = (root_disk_bytes / (1024 * 1024 * 1024)).try_into().context("Too much disk space")?;
        if reserved.disk_space_gb > disk_space_gb {
            bail!("Reserved disk space ({}) exceeds total disk space ({disk_space_gb})", reserved.disk_space_gb);
        }

        let cpus = sys.cpus().len() as u32;
        if reserved.cpus > cpus {
            bail!("Reserved CPUs ({}) exceed total CPUs ({cpus})", reserved.cpus);
        }

        let gpus = Self::find_gpus().await?;
        Ok(Self {
            hostname,
            memory_mb,
            reserved_memory_mb: reserved.memory_mb,
            disk_space_gb,
            reserved_disk_space_gb: reserved.disk_space_gb,
            cpus,
            reserved_cpus: reserved.cpus,
            gpus,
        })
    }

    pub(crate) fn available_cpus(&self) -> u32 {
        self.cpus.saturating_sub(self.reserved_cpus)
    }

    pub(crate) fn available_memory_mb(&self) -> u32 {
        self.memory_mb.saturating_sub(self.reserved_memory_mb)
    }

    pub(crate) fn available_disk_space_gb(&self) -> u32 {
        self.disk_space_gb.saturating_sub(self.reserved_disk_space_gb)
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

        Ok(Some(Gpus::new(H100_MODEL, addresses)))
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

    pub fn find_public_ip() -> anyhow::Result<Ipv4Addr> {
        let networks = Networks::new_with_refreshed_list();
        for (_, network) in &networks {
            for network in network.ip_networks() {
                let IpAddr::V4(addr) = network.addr else {
                    debug!("Ignoring IPv6 address {}", network.addr);
                    continue;
                };
                if addr.is_public() {
                    info!("Found public IPv4 address: {addr}");
                    return Ok(addr);
                }
            }
        }
        bail!("not public addresses available");
    }

    pub async fn adjust_gpu_assignment(&self, provider: &dyn RepositoryProvider) -> anyhow::Result<()> {
        info!("Validating GPU assignment");
        let mut repo = provider.workloads(Default::default()).await?;
        let workloads = repo.list().await?;
        let gpus: BTreeSet<_> = self.gpus.as_ref().map(|g| g.addresses.iter().collect()).unwrap_or_default();
        let assigned_gpus: BTreeSet<_> = workloads.iter().flat_map(|w| w.gpus.iter()).collect();
        if gpus == assigned_gpus {
            info!("Assigned GPUs match the current assignment");
            return Ok(());
        } else if gpus.len() != assigned_gpus.len() {
            // We can't do anything if GPUs have disappeared
            bail!("Have {} assigned GPUs but only {} available", assigned_gpus.len(), gpus.len());
        }
        info!("GPU addresses have changed, re-assigning them");

        let mut gpus = gpus.into_iter().cloned();
        let mut repo = provider.workloads(ProviderMode::Transactional).await?;
        for workload in workloads {
            if workload.gpus.is_empty() {
                continue;
            }

            let new_gpus = gpus.by_ref().take(workload.gpus.len()).collect();
            info!("Assigning GPUs for workload {} from {:?} to {:?}", workload.id, workload.gpus, new_gpus);
            repo.set_gpus(workload.id, new_gpus).await?;
        }
        repo.commit().await?;
        Ok(())
    }
}

trait IsPublic {
    fn is_public(&self) -> bool;
}

impl IsPublic for Ipv4Addr {
    fn is_public(&self) -> bool {
        // TODO: use `Ipv4Addr::is_global` when stabilized
        let octets = self.octets();

        // 127.0.0.0/8
        if octets[0] == 127 {
            return false;
        }

        // 10.0.0.0/8
        if octets[0] == 10 {
            return false;
        }

        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return false;
        }

        // 169.254.1.0 - 169.254.254.255
        if octets[0] == 169 && octets[1] == 254 && octets[2] != 0 && octets[2] != 255 {
            return false;
        }

        // 172.16.0.0/12
        if octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31 {
            return false;
        }

        // 100.64.0.0/10
        if octets[0] == 100 && (octets[1] & 0xc0) == 64 {
            return false;
        }

        // 100.64.0.0/10
        if octets[0] == 100 && (octets[1] & 0xc0) == 64 {
            return false;
        }

        true
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
pub struct Gpus {
    pub model: String,
    pub addresses: Vec<GpuAddress>,
}

impl Gpus {
    pub(crate) fn new<S: Into<String>, I: Into<Vec<GpuAddress>>>(model: S, addresses: I) -> Self {
        Self { model: model.into(), addresses: addresses.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{
        sqlite::{SqliteDb, SqliteRepositoryProvider},
        workload::Workload,
    };
    use uuid::Uuid;

    fn make_workload(domain: &str, gpus: &[GpuAddress]) -> Workload {
        Workload {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            env_vars: Default::default(),
            files: Default::default(),
            docker_credentials: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 1.try_into().unwrap(),
            gpus: gpus.iter().cloned().collect(),
            ports: [150, 151, 152],
            domain: domain.into(),
            enabled: true,
        }
    }

    #[tokio::test]
    async fn gather() {
        let resources = SystemResources::gather(Default::default()).await.expect("failed to gather resources");
        assert!(resources.cpus > 0);
        assert!(resources.disk_space_gb > 0);
    }

    #[tokio::test]
    async fn gather_too_much_reserved_cpu() {
        let reserved = ReservedResourcesConfig { cpus: 1024, memory_mb: 0, disk_space_gb: 0 };
        SystemResources::gather(reserved).await.expect_err("gathering did not fail");
    }

    #[tokio::test]
    async fn gather_too_much_reserved_memory() {
        let reserved = ReservedResourcesConfig { cpus: 0, memory_mb: 1024 * 200, disk_space_gb: 0 };
        SystemResources::gather(reserved).await.expect_err("gathering did not fail");
    }

    #[tokio::test]
    async fn gather_too_much_reserved_disk() {
        let reserved = ReservedResourcesConfig { cpus: 0, memory_mb: 0, disk_space_gb: 100_000 };
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

    #[tokio::test]
    async fn adjust_gpus() {
        let mut resources = SystemResources::gather(Default::default()).await.expect("failed to gather");
        resources.gpus = Some(Gpus { model: "foo".into(), addresses: vec!["aa".into(), "bb".into()] });
        let workloads = vec![make_workload("a.com", &["bb".into()]), make_workload("b.com", &["cc".into()])];
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let provider = SqliteRepositoryProvider::new(db);
        {
            let mut repo = provider.workloads(Default::default()).await.expect("failed to list workloads");
            for workload in &workloads {
                repo.create(workload.clone()).await.expect("failed to create");
            }
        }
        resources.adjust_gpu_assignment(&provider).await.expect("failed to adjust");
        let mut repo = provider.workloads(Default::default()).await.expect("failed to list workloads");
        let updated_workloads = repo.list().await.expect("failed to list");
        let gpus: BTreeSet<_> = updated_workloads.iter().flat_map(|w| w.gpus.iter().cloned()).collect();
        let expected_gpus = ["aa".into(), "bb".into()].into_iter().collect();
        assert_eq!(gpus, expected_gpus);
    }

    #[tokio::test]
    async fn no_gpus() {
        let mut resources = SystemResources::gather(Default::default()).await.expect("failed to gather");
        resources.gpus = Some(Gpus { model: "foo".into(), addresses: vec![] });
        let workloads = vec![make_workload("a.com", &["aa".into()])];
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let provider = SqliteRepositoryProvider::new(db);
        {
            let mut repo = provider.workloads(Default::default()).await.expect("failed to list workloads");
            for workload in &workloads {
                repo.create(workload.clone()).await.expect("failed to create");
            }
        }
        resources.adjust_gpu_assignment(&provider).await.expect_err("adjustment succeeded");
    }

    #[tokio::test]
    async fn same_gpus() {
        let mut resources = SystemResources::gather(Default::default()).await.expect("failed to gather");
        resources.gpus = Some(Gpus { model: "foo".into(), addresses: vec!["aa".into()] });
        let workloads = vec![make_workload("a.com", &["aa".into()])];
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let provider = SqliteRepositoryProvider::new(db);
        {
            let mut repo = provider.workloads(Default::default()).await.expect("failed to list workloads");
            for workload in &workloads {
                repo.create(workload.clone()).await.expect("failed to create");
            }
        }
        resources.adjust_gpu_assignment(&provider).await.expect("adjustment failed");

        let mut repo = provider.workloads(Default::default()).await.expect("failed to list workloads");
        let updated_workloads = repo.list().await.expect("failed to list");
        assert_eq!(updated_workloads, workloads);
    }
}
