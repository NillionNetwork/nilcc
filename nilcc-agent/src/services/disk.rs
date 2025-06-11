use crate::qemu_client::HardDiskFormat;
use anyhow::{bail, Context};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[async_trait]
pub trait DiskService: Send + Sync {
    /// Create a disk at the given path with the given format.
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: usize) -> anyhow::Result<()>;
}

pub struct DefaultDiskService {
    qemu_img_path: PathBuf,
}

impl DefaultDiskService {
    pub fn new(qemu_img_path: PathBuf) -> Self {
        Self { qemu_img_path }
    }
}

#[async_trait]
impl DiskService for DefaultDiskService {
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: usize) -> anyhow::Result<()> {
        let format = format.to_string();
        let args = ["create", "-f", &format, &path.to_string_lossy(), &format!("{size_gib}G")];
        let output = Command::new(&self.qemu_img_path)
            .args(args)
            .output()
            .await
            .context("Failed to invoke qemu-img (is qemu-img path correct?)")?;
        if output.status.success() {
            Ok(())
        } else {
            bail!("qemu-img failed: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
