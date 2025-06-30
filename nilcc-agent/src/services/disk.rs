use crate::{
    clients::qemu::HardDiskFormat,
    iso::{IsoMaker, IsoSpec},
};
use anyhow::{bail, Context};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DiskService: Send + Sync {
    /// Create a disk at the given path with the given format.
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: u16) -> anyhow::Result<()>;

    /// Create the ISO for an application.
    async fn create_application_iso(&self, path: &Path, spec: IsoSpec) -> anyhow::Result<DockerComposeHash>;
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
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: u16) -> anyhow::Result<()> {
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

    async fn create_application_iso(&self, path: &Path, spec: IsoSpec) -> anyhow::Result<DockerComposeHash> {
        let meta = IsoMaker.create_application_iso(path, spec).await?;
        let compose_hash = hex::encode(meta.docker_compose_hash);
        Ok(DockerComposeHash(compose_hash))
    }
}

#[derive(Clone, Debug)]
pub struct DockerComposeHash(pub String);
