use anyhow::bail;
use tokio::process::Command;

pub const SUPPORTED_GPU_MODEL: &str = "H100";
const NVIDIA_GPU_VENDOR_ID: &str = "10de";

pub struct GpuGroup {
    pub model: String,
    pub addresses: Vec<String>,
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
            bail!(format!("Unsupported NVIDIA GPU found. All GPUs must be {SUPPORTED_GPU_MODEL}. Detected: {line}"));
        }
    }

    addresses.sort();

    Ok(Some(GpuGroup { model: SUPPORTED_GPU_MODEL.to_string(), addresses }))
}
