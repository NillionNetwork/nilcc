use crate::repositories::artifacts::Artifacts;
use crate::repositories::sqlite::ProviderMode;
use crate::repositories::sqlite::RepositoryProvider;
use crate::routes::Json;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use async_trait::async_trait;
use axum::response::IntoResponse;
use axum::response::Response;
use chrono::{DateTime, Utc};
use nilcc_agent_models::errors::RequestHandlerError;
use nilcc_artifacts::downloader::{ArtifactsDownloader, FileDownloader};
use nilcc_artifacts::VmType;
use reqwest::StatusCode;
use std::collections::HashSet;
use std::env;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::process::Output;
use std::process::Stdio;
use std::{path::PathBuf, sync::Arc};
use strum::EnumDiscriminants;
use tempfile::NamedTempFile;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::warn;
use tracing::{error, info};

const UPDATER_SCRIPT: &[u8] = include_bytes!("../../resources/update.sh");

#[async_trait]
pub trait UpgradeService: Send + Sync {
    async fn upgrade_artifacts(&self, version: String, vm_types: Vec<VmType>) -> Result<(), UpgradeError>;
    async fn upgrade_agent(&self, version: String) -> Result<(), UpgradeError>;
    async fn cleanup_artifacts(&self) -> Result<Vec<String>, CleanupError>;
    async fn artifacts_upgrade_state(&self) -> UpgradeState;
    async fn artifacts_version(&self) -> anyhow::Result<String>;
    async fn agent_upgrade_state(&self) -> UpgradeState;
    fn agent_version(&self) -> String;
}

#[derive(Debug, EnumDiscriminants, thiserror::Error)]
pub enum UpgradeError {
    #[error("invalid version")]
    InvalidVersion,

    #[error("version already exists in agent")]
    ExistingVersion,

    #[error("an upgrade to version {0} is already in progress")]
    ActiveUpgrade(String),

    #[error("internal error")]
    Internal,
}

impl IntoResponse for UpgradeError {
    fn into_response(self) -> Response {
        let discriminant = UpgradeErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::InvalidVersion => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::ActiveUpgrade(_) | Self::ExistingVersion => (StatusCode::PRECONDITION_FAILED, self.to_string()),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}

#[derive(Debug, EnumDiscriminants, thiserror::Error)]
pub enum CleanupError {
    #[error("internal error")]
    Internal,
}

impl IntoResponse for CleanupError {
    fn into_response(self) -> Response {
        let discriminant = CleanupErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}

pub struct DefaultUpgradeServiceArgs {
    pub repository_provider: Arc<dyn RepositoryProvider>,
    pub config_file_path: PathBuf,
    pub cvm_artifacts_path: PathBuf,
}

pub struct DefaultUpgradeService {
    artifacts: Arc<Mutex<UpgradeState>>,
    agent: Arc<Mutex<UpgradeState>>,
    config_file_path: PathBuf,
    cvm_artifacts_path: PathBuf,
    repository_provider: Arc<dyn RepositoryProvider>,
}

impl DefaultUpgradeService {
    pub fn new(args: DefaultUpgradeServiceArgs) -> Self {
        let DefaultUpgradeServiceArgs { repository_provider, config_file_path, cvm_artifacts_path } = args;
        Self {
            artifacts: Default::default(),
            agent: Default::default(),
            repository_provider,
            config_file_path,
            cvm_artifacts_path,
        }
    }
}

#[async_trait]
impl UpgradeService for DefaultUpgradeService {
    async fn upgrade_artifacts(&self, version: String, vm_types: Vec<VmType>) -> Result<(), UpgradeError> {
        let mut current = self.artifacts.lock().await;
        match &*current {
            UpgradeState::Upgrading { metadata, .. } => {
                return Err(UpgradeError::ActiveUpgrade(metadata.version.clone()))
            }
            UpgradeState::None | UpgradeState::Done { .. } => (),
        };
        let mut repo = self.repository_provider.artifacts(Default::default()).await.map_err(|e| {
            error!("Failed to get repository: {e}");
            UpgradeError::Internal
        })?;
        let exists = repo.exists(&version).await.map_err(|e| {
            error!("Failed to check if version exists: {e}");
            UpgradeError::Internal
        })?;
        if exists {
            return Err(UpgradeError::ExistingVersion);
        }

        let downloader = ArtifactsDownloader::new(version.clone(), vm_types.clone());
        downloader.validate_exists().await.map_err(|_| UpgradeError::InvalidVersion)?;

        info!("Initiating artifacts upgrade to version {version}");
        let metadata = UpgradeMetadata { version: version.clone(), started_at: Utc::now(), vm_types };
        let state = self.artifacts.clone();
        *current = UpgradeState::Upgrading { metadata };

        let target_path = self.cvm_artifacts_path.join(&version);
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

    async fn cleanup_artifacts(&self) -> Result<Vec<String>, CleanupError> {
        let used_versions: HashSet<_> = {
            let mut repo = self.repository_provider.workloads(Default::default()).await.map_err(|e| {
                error!("Failed to get repository: {e}");
                CleanupError::Internal
            })?;
            let workloads = repo.list().await.map_err(|e| {
                error!("Failed to list workloads: {e}");
                CleanupError::Internal
            })?;
            workloads.into_iter().map(|w| w.artifacts_version).collect()
        };

        let mut repo = self.repository_provider.artifacts(Default::default()).await.map_err(|e| {
            error!("Failed to get repository: {e}");
            CleanupError::Internal
        })?;
        let versions = repo.list().await.map_err(|e| {
            error!("Failed to list versions: {e}");
            CleanupError::Internal
        })?;
        info!("Initiating cleanup for {} versions", versions.len());

        let mut deleted_versions = Vec::new();
        for version in versions {
            let Artifacts { version, current, .. } = version;
            if current {
                info!("Not deleting version {version} because it's the current one");
                continue;
            }
            if used_versions.contains(&version) {
                info!("Not deleting version {version} because there's workloads using it");
                continue;
            }

            info!("Deleting version {version} in database");
            repo.delete(&version).await.map_err(|e| {
                error!("Failed to delete version {version} from database: {e}");
                CleanupError::Internal
            })?;

            let path = self.cvm_artifacts_path.join(&version);
            fs::remove_dir_all(&path).await.map_err(|e| {
                error!("Failed to delete artifacts for version {version}: {e}");
                CleanupError::Internal
            })?;
            info!("Deleted artifacts for version {version}");
            deleted_versions.push(version);
        }
        Ok(deleted_versions)
    }

    async fn upgrade_agent(&self, version: String) -> Result<(), UpgradeError> {
        let mut current = self.agent.lock().await;
        match &*current {
            UpgradeState::Upgrading { metadata, .. } => {
                return Err(UpgradeError::ActiveUpgrade(metadata.version.clone()))
            }
            UpgradeState::None | UpgradeState::Done { .. } => (),
        };
        let url_path = agent_url(&version);
        FileDownloader::default().exists(&url_path).await.map_err(|e| {
            warn!("Failed to check if agent exists: {e:#}");
            UpgradeError::InvalidVersion
        })?;

        let agent_path = env::current_exe().map_err(|e| {
            error!("Failed to get agent binary path: {e}");
            UpgradeError::Internal
        })?;
        let permissions = Permissions::from_mode(0o700);
        let temp_agent_path =
            tempfile::Builder::new().prefix("nilcc-agent").permissions(permissions.clone()).tempfile().map_err(
                |e| {
                    error!("Failed to create tempfile: {e}");
                    UpgradeError::Internal
                },
            )?;
        let updater =
            tempfile::Builder::new().prefix("nilcc-agent-updater").permissions(permissions).tempfile().map_err(
                |e| {
                    error!("Failed to create tempfile: {e}");
                    UpgradeError::Internal
                },
            )?;
        fs::write(updater.path(), UPDATER_SCRIPT).await.map_err(|e| {
            error!("Failed to write updater script: {e}");
            UpgradeError::Internal
        })?;

        let metadata =
            UpgradeMetadata { version: version.clone(), started_at: Utc::now(), vm_types: Default::default() };
        *current = UpgradeState::Upgrading { metadata };
        info!("Initiating agent upgrade to version {version}");

        let worker = AgentUpgradeWorker {
            temp_agent_path,
            updater,
            state: self.agent.clone(),
            version,
            agent_path,
            config_path: self.config_file_path.clone(),
        };
        tokio::spawn(async move { worker.run().await });
        Ok(())
    }

    async fn artifacts_upgrade_state(&self) -> UpgradeState {
        self.artifacts.lock().await.clone()
    }

    async fn artifacts_version(&self) -> anyhow::Result<String> {
        let mut repo = self.repository_provider.artifacts(Default::default()).await?;
        let version = repo.get().await?;
        Ok(version.ok_or_else(|| anyhow!("no version in db"))?.version)
    }

    async fn agent_upgrade_state(&self) -> UpgradeState {
        self.agent.lock().await.clone()
    }

    fn agent_version(&self) -> String {
        crate::version::agent_version().into()
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
                Some(format!("{e:#}"))
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
        let artifacts = self.downloader.download(&self.target_path).await?;
        info!("Upgrade to version {version} successful");

        let mut repo = self
            .repository_provider
            .artifacts(ProviderMode::Transactional)
            .await
            .context("Failed to get repository")?;
        repo.set(version, &artifacts.metadata).await.context("Failed to set version")?;
        repo.commit().await?;
        Ok(())
    }
}

struct AgentUpgradeWorker {
    temp_agent_path: NamedTempFile,
    updater: NamedTempFile,
    state: Arc<Mutex<UpgradeState>>,
    version: String,
    agent_path: PathBuf,
    config_path: PathBuf,
}

impl AgentUpgradeWorker {
    async fn run(self) {
        let result = self.perform_upgrade().await;

        let error = match result {
            Ok(_) => None,
            Err(e) => {
                error!("Failed to upgrade agent: {e:#}");
                Some(format!("{e:#}"))
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
        let Self { temp_agent_path, updater, version, agent_path, config_path, .. } = self;
        let url_path = agent_url(version);
        info!("Downloading agent {url_path} to {}", temp_agent_path.path().display());
        FileDownloader::default()
            .download(&url_path, temp_agent_path.path())
            .await
            .context("Failed to download agent")?;

        info!("Starting updater at {}", updater.path().display());
        let child = Command::new(updater.path())
            .arg(temp_agent_path.path())
            .arg(agent_path)
            .arg(config_path)
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn process")?;
        let output = child.wait_with_output().await.context("Failed to wait for child process")?;
        let Output { status, stdout, stderr } = output;
        if status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&stderr);
            let stdout = String::from_utf8_lossy(&stdout);
            bail!("agent updater failed with status code {status}: stdout = {stdout}, stderr = {stderr}");
        }
    }
}

fn agent_url(version: &str) -> String {
    format!("/{version}/nilcc-agent/x86-64/nilcc-agent")
}
