use crate::{build_info::get_agent_version, data_schemas::MetalInstanceDetails};
use anyhow::{bail, Context};
use sysinfo::{Disks, System};
use tokio::process::Command;
use tracing::info;

const SUPPORTED_GPU_MODEL: &str = "H100";
const NVIDIA_GPU_VENDOR_ID: &str = "10de";

pub struct SystemResources;

impl SystemResources {
    // Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
    pub async fn gather() -> anyhow::Result<MetalInstanceDetails> {
        info!("Gathering metal instance details");

        let sys = System::new_all();

        let hostname = System::host_name().context("Failed to get hostname from sysinfo")?;
        let memory = sys.total_memory() / (1024 * 1024 * 1024);
        let disks = Disks::new_with_refreshed_list();
        let mut root_disk_bytes = 0;
        for disk in disks.list() {
            if disk.mount_point().as_os_str() == "/" {
                root_disk_bytes = disk.total_space();
            }
        }
        let disk = root_disk_bytes / (1024 * 1024 * 1024);
        let cpu = sys.cpus().len() as u32;
        let gpu_group = Self::find_gpus().await?;

        let (gpu_model, gpu_count) =
            gpu_group.map(|group| (Some(group.model.clone()), Some(group.addresses.len() as u32))).unwrap_or_default();

        let details = MetalInstanceDetails {
            agent_version: get_agent_version().to_string(),
            hostname,
            memory,
            disk,
            cpu,
            gpu: gpu_count,
            gpu_model,
        };

        Ok(details)
    }

    /// Finds supported NVIDIA GPUs
    pub(crate) async fn find_gpus() -> anyhow::Result<Option<GpuGroup>> {
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
                    addresses.push(bdf.to_string());
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

        Ok(Some(GpuGroup { model: SUPPORTED_GPU_MODEL.to_string(), addresses }))
    }
}

pub(crate) struct GpuGroup {
    pub(crate) model: String,
    pub(crate) addresses: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn foo() {
        let resources = SystemResources::gather().await.expect("failed to gather resources");
        assert!(resources.cpu > 0);
        assert!(resources.disk > 0);
    }
}
