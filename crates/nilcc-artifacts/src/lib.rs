use anyhow::anyhow;
use anyhow::Context;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::BufWriter;
use tracing::debug;
use tracing::info;

pub const S3_BUCKET_URL: &str = "https://nilcc.s3-accelerate.amazonaws.com";

#[derive(Clone, Debug)]
pub struct ArtifactsDownloader {
    version: String,
    vm_types: Vec<VmType>,
    artifacts_url: String,
    disk_images: bool,
    always_download: bool,
}

impl ArtifactsDownloader {
    pub fn new(version: String, vm_types: Vec<VmType>) -> Self {
        Self { version, vm_types, artifacts_url: S3_BUCKET_URL.into(), disk_images: true, always_download: true }
    }

    pub fn with_artifacts_url(mut self, artifacts_url: String) -> Self {
        self.artifacts_url = artifacts_url;
        self
    }

    pub fn without_disk_images(mut self) -> Self {
        self.disk_images = false;
        self
    }

    pub fn without_artifact_overwrite(mut self) -> Self {
        self.always_download = false;
        self
    }

    pub async fn validate_exists(&self) -> anyhow::Result<()> {
        let Self { version, artifacts_url, .. } = self;
        let url = format!("{artifacts_url}/{version}/metadata.json");
        reqwest::get(url).await?.error_for_status()?;
        Ok(())
    }

    pub async fn download(&self, target_dir: &Path) -> anyhow::Result<Artifacts> {
        info!("Downloading artifacts to {}", target_dir.display());

        let ovmf_path = self.download_artifact("vm_images/ovmf/OVMF.fd", target_dir).await?;
        let initrd_path = self.download_artifact("initramfs/initramfs.cpio.gz", target_dir).await?;
        let mut type_artifacts = HashMap::new();
        for vm_type in &self.vm_types {
            let kernel_path =
                self.download_artifact(&format!("vm_images/kernel/{vm_type}-vmlinuz"), target_dir).await?;
            let filesystem_root_hash_path =
                self.download_artifact(&format!("vm_images/cvm-{vm_type}-verity/root-hash"), target_dir).await?;
            let hex_filesystem_root_hash = fs::read_to_string(filesystem_root_hash_path)
                .await
                .context("reading local copy of filesystem root hash")?;
            let mut filesystem_root_hash: [u8; 32] = [0; 32];
            hex::decode_to_slice(hex_filesystem_root_hash.trim(), &mut filesystem_root_hash)
                .context("decoding filesystem root hash")?;
            let (base_disk, verity_disk) = match &self.disk_images {
                true => {
                    let base_disk =
                        self.download_artifact(&format!("vm_images/cvm-{vm_type}.qcow2"), target_dir).await?;
                    let verity_disk = self
                        .download_artifact(&format!("vm_images/cvm-{vm_type}-verity/verity-hash-dev"), target_dir)
                        .await?;
                    (Some(base_disk), Some(verity_disk))
                }
                false => (None, None),
            };
            let artifacts = VmTypeArtifacts { kernel_path, base_disk, verity_disk, filesystem_root_hash };
            type_artifacts.insert(*vm_type, artifacts);
        }
        Ok(Artifacts { ovmf_path, initrd_path, type_artifacts })
    }

    async fn download_artifact(&self, artifact_name: &str, target_dir: &Path) -> anyhow::Result<PathBuf> {
        let local_path = target_dir.join(artifact_name);
        if local_path.exists() {
            if self.always_download {
                info!("Artifact {artifact_name} already exists, overwriting it");
            } else {
                info!("Not downloading {artifact_name} because it already exists in cache directory");
                return Ok(local_path);
            }
        }
        info!("Downloading {artifact_name} into {}", local_path.display());
        let parent = local_path.parent().ok_or_else(|| anyhow!("path has no parent"))?;
        fs::create_dir_all(parent).await.context("creating cache directory")?;

        let version = &self.version;
        let remote_path = format!("/{version}/{artifact_name}");
        self.download_object(&remote_path, &local_path)
            .await
            .map_err(|e| anyhow!("failed to download {remote_path}: {e}"))?;
        Ok(local_path)
    }

    async fn download_object(&self, url_path: &str, target_path: &Path) -> anyhow::Result<()> {
        FileDownloader { artifacts_url: &self.artifacts_url }.download(url_path, target_path).await
    }
}

pub struct FileDownloader<'a> {
    artifacts_url: &'a str,
}

impl Default for FileDownloader<'static> {
    fn default() -> Self {
        Self { artifacts_url: S3_BUCKET_URL }
    }
}

impl FileDownloader<'_> {
    pub async fn exists(&self, url_path: &str) -> anyhow::Result<()> {
        let url = format!("{}{url_path}", self.artifacts_url);
        reqwest::Client::new().head(url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn download(&self, url_path: &str, target_path: &Path) -> anyhow::Result<()> {
        let url = format!("{}{url_path}", self.artifacts_url);
        let result = reqwest::get(url).await?.error_for_status()?;
        let mut stream = result.bytes_stream();
        let file = File::create(target_path).await.context("Failed to create target file")?;
        let mut file = BufWriter::new(file);
        while let Some(bytes) = stream.next().await {
            let bytes = bytes?;
            debug!("Writing {} bytes chunk", bytes.len());
            file.write_all(&bytes).await?;
        }
        file.flush().await?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VmType {
    Gpu,
    Cpu,
}

impl fmt::Display for VmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu => write!(f, "gpu"),
            Self::Cpu => write!(f, "cpu"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Artifacts {
    pub ovmf_path: PathBuf,
    pub initrd_path: PathBuf,
    pub type_artifacts: HashMap<VmType, VmTypeArtifacts>,
}

#[derive(Clone, Debug)]
pub struct VmTypeArtifacts {
    pub kernel_path: PathBuf,
    pub base_disk: Option<PathBuf>,
    pub verity_disk: Option<PathBuf>,
    pub filesystem_root_hash: [u8; 32],
}
