use crate::Artifacts;
use crate::VmType;
use crate::metadata::ArtifactsMetadata;
use futures_util::StreamExt;
use sha2::Digest;
use sha2::Sha256;
use std::io;
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

    pub async fn validate_exists(&self) -> Result<(), DownloadError> {
        let Self { version, artifacts_url, .. } = self;
        let url = format!("{artifacts_url}/{version}/metadata.json");
        reqwest::get(url).await?.error_for_status()?;
        Ok(())
    }

    pub async fn download(&self, target_dir: &Path) -> Result<Artifacts, DownloadError> {
        info!("Downloading artifacts to {}", target_dir.display());
        let artifact_metadata = self.fetch_metadata().await?;
        let metadata = &artifact_metadata.decoded;
        let metadata_path = target_dir.join("metadata.json");
        self.download_artifact(&metadata.ovmf.path, target_dir).await?;
        self.download_artifact(&metadata.initrd.path, target_dir).await?;
        for vm_type in &self.vm_types {
            let metadata = metadata.cvm.images.resolve(*vm_type);
            self.download_artifact(&metadata.kernel.path, target_dir).await?;
            if self.disk_images {
                self.download_artifact(&metadata.disk.artifact.path, target_dir).await?;
                self.download_artifact(&metadata.verity.disk.path, target_dir).await?;
            }
        }
        fs::write(&metadata_path, artifact_metadata.raw).await.map_err(DownloadError::TargetFile)?;
        Ok(Artifacts { metadata: artifact_metadata.decoded, metadata_hash: artifact_metadata.hash })
    }

    async fn download_artifact(&self, artifact_name: &str, target_dir: &Path) -> Result<PathBuf, DownloadError> {
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
        let parent = local_path.parent().ok_or_else(|| DownloadError::NoParent)?;
        fs::create_dir_all(parent).await.map_err(DownloadError::TargetDirectory)?;

        let version = &self.version;
        let remote_path = format!("/{version}/{artifact_name}");
        self.download_object(&remote_path, &local_path).await?;
        Ok(local_path)
    }

    async fn fetch_metadata(&self) -> Result<Metadata, DownloadError> {
        let version = &self.version;
        let url = format!("{}/{version}/metadata.json", self.artifacts_url);
        let response = reqwest::get(url).await?.error_for_status()?;
        let raw_metadata = response.text().await?;
        let metadata_hash = Sha256::digest(&raw_metadata).into();
        let metadata = serde_json::from_str(&raw_metadata).map_err(DownloadError::DecodeMetadata)?;
        Ok(Metadata { raw: raw_metadata, decoded: metadata, hash: metadata_hash })
    }

    async fn download_object(&self, url_path: &str, target_path: &Path) -> Result<(), DownloadError> {
        FileDownloader { artifacts_url: &self.artifacts_url }.download(url_path, target_path).await
    }
}

struct Metadata {
    raw: String,
    decoded: ArtifactsMetadata,
    hash: [u8; 32],
}

#[derive(thiserror::Error, Debug)]
pub enum DownloadError {
    #[error("no parent in target path")]
    NoParent,

    #[error("could not create target directory: {0}")]
    TargetDirectory(io::Error),

    #[error("could not write target file: {0}")]
    TargetFile(io::Error),

    #[error("could not download file: {0}")]
    Download(#[from] reqwest::Error),

    #[error("failed to decode metadata: {0}")]
    DecodeMetadata(serde_json::Error),
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
    pub async fn exists(&self, url_path: &str) -> Result<(), DownloadError> {
        let url = format!("{}{url_path}", self.artifacts_url);
        reqwest::Client::new().head(url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn download(&self, url_path: &str, target_path: &Path) -> Result<(), DownloadError> {
        let url = format!("{}{url_path}", self.artifacts_url);
        let result = reqwest::get(url).await?.error_for_status()?;
        let mut stream = result.bytes_stream();
        let file = File::create(target_path).await.map_err(DownloadError::TargetFile)?;
        let mut file = BufWriter::new(file);
        while let Some(bytes) = stream.next().await {
            let bytes = bytes?;
            debug!("Writing {} bytes chunk", bytes.len());
            file.write_all(&bytes).await.map_err(DownloadError::TargetFile)?;
        }
        file.flush().await.map_err(DownloadError::TargetFile)?;
        Ok(())
    }
}
