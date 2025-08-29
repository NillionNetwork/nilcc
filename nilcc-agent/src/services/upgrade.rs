use crate::repositories::sqlite::RepositoryProvider;
use anyhow::anyhow;
use anyhow::Context;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nilcc_artifacts::{ArtifactsDownloader, VmType};
use std::{path::PathBuf, sync::Arc};
use strum::EnumDiscriminants;
use tokio::sync::Mutex;
use tracing::{error, info};

#[async_trait]
pub trait UpgradeService: Send + Sync {
    async fn upgrade_artifacts(
        &self,
        version: String,
        vm_types: Vec<VmType>,
        target_path: PathBuf,
    ) -> Result<(), UpgradeError>;

    async fn artifacts_upgrade_state(&self) -> UpgradeState;
    async fn artifacts_version(&self) -> anyhow::Result<String>;
}

#[derive(Debug, EnumDiscriminants, thiserror::Error)]
pub enum UpgradeError {
    #[error("invalid version")]
    InvalidVersion,

    #[error("an upgrade to version {0} is already in progress")]
    ActiveUpgrade(String),
}

pub struct DefaultUpgradeServiceArgs {
    pub repository_provider: Arc<dyn RepositoryProvider>,
}

pub struct DefaultUpgradeService {
    artifacts: Arc<Mutex<UpgradeState>>,
    repository_provider: Arc<dyn RepositoryProvider>,
}

impl DefaultUpgradeService {
    pub fn new(args: DefaultUpgradeServiceArgs) -> Self {
        let DefaultUpgradeServiceArgs { repository_provider } = args;
        Self { artifacts: Default::default(), repository_provider }
    }
}

#[async_trait]
impl UpgradeService for DefaultUpgradeService {
    async fn upgrade_artifacts(
        &self,
        version: String,
        vm_types: Vec<VmType>,
        target_path: PathBuf,
    ) -> Result<(), UpgradeError> {
        let mut current = self.artifacts.lock().await;
        match &*current {
            UpgradeState::Upgrading { metadata, .. } => {
                return Err(UpgradeError::ActiveUpgrade(metadata.version.clone()))
            }
            UpgradeState::None | UpgradeState::Done { .. } => (),
        };
        let downloader = ArtifactsDownloader::new(version.clone(), vm_types.clone());
        downloader.validate_exists().await.map_err(|_| UpgradeError::InvalidVersion)?;

        info!("Initiating upgrade to version {version}");
        let metadata = UpgradeMetadata { version: version.clone(), started_at: Utc::now(), vm_types };
        let state = self.artifacts.clone();
        *current = UpgradeState::Upgrading { metadata };

        let worker = ArtifactUpgradeWorker {
            downloader,
            target_path,
            state,
            version,
            repository_provider: self.repository_provider.clone(),
        };
        tokio::spawn(async move { worker.run().await });
        Ok(())
    }

    async fn artifacts_upgrade_state(&self) -> UpgradeState {
        self.artifacts.lock().await.clone()
    }

    async fn artifacts_version(&self) -> anyhow::Result<String> {
        let mut repo = self.repository_provider.artifacts_version(Default::default()).await?;
        let version = repo.get().await?;
        version.ok_or_else(|| anyhow!("no version in db"))
    }
}

#[derive(Clone, Default, Debug)]
pub enum UpgradeState {
    #[default]
    None,
    Upgrading {
        metadata: UpgradeMetadata,
    },
    Done {
        metadata: UpgradeMetadata,
        finished_at: DateTime<Utc>,
        error: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct UpgradeMetadata {
    pub version: String,
    pub started_at: DateTime<Utc>,
    pub vm_types: Vec<VmType>,
}

struct ArtifactUpgradeWorker {
    downloader: ArtifactsDownloader,
    target_path: PathBuf,
    state: Arc<Mutex<UpgradeState>>,
    version: String,
    repository_provider: Arc<dyn RepositoryProvider>,
}

impl ArtifactUpgradeWorker {
    async fn run(self) {
        let version = &self.version;
        let error = match self.perform_upgrade().await {
            Ok(_) => None,
            Err(e) => {
                error!("Failed to upgrade to version {version}: {e}");
                Some(e.to_string())
            }
        };
        let mut state = self.state.lock().await;
        match &*state {
            UpgradeState::None => {
                error!("Not running any upgrades");
            }
            UpgradeState::Done { .. } => {
                error!("Upgrade is marked as completed already");
            }
            UpgradeState::Upgrading { metadata } => {
                *state = UpgradeState::Done { metadata: metadata.clone(), finished_at: Utc::now(), error }
            }
        };
    }

    async fn perform_upgrade(&self) -> anyhow::Result<()> {
        let version = &self.version;
        self.downloader.download(&self.target_path).await?;
        info!("Upgrade to version {version} successful");
        let mut repo =
            self.repository_provider.artifacts_version(Default::default()).await.context("Failed to get repository")?;
        repo.set(version).await.context("Failed to set version")?;
        Ok(())
    }
}
