use crate::{heartbeat::HeartbeatEmitterHandle, monitors::EventHolder};
use axum::{
    Router,
    extract::State,
    routing::{get, post},
};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub(crate) mod config;
pub(crate) mod containers;
pub(crate) mod health;
pub(crate) mod system;

#[derive(Default)]
pub enum SystemState {
    #[default]
    WaitingBootstrap,
    Starting,
    Ready,
}

#[derive(Clone)]
pub struct BootstrapContext {
    pub system_docker_compose: PathBuf,
    pub user_docker_compose: PathBuf,
    pub user_docker_compose_sha256: [u8; 32],
    pub external_files: PathBuf,
    pub caddy_config: PathBuf,
    pub docker_config: PathBuf,
    pub version: String,
    pub vm_type: VmType,
    pub iso_mount: PathBuf,
    pub event_holder: EventHolder,
    pub cpus: u64,
    pub gpus: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmType {
    Cpu,
    Gpu,
}

impl fmt::Display for VmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
        }
    }
}

pub struct AppState {
    pub docker: Docker,
    pub context: BootstrapContext,
    pub system_state: Arc<Mutex<SystemState>>,
    pub log_path: PathBuf,
    pub heartbeat_handle: Arc<Mutex<Option<HeartbeatEmitterHandle>>>,
}

pub(crate) type SharedState = State<Arc<AppState>>;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new().nest(
        "/api/v1",
        Router::new()
            .route("/health", get(health::handler))
            .route("/config/heartbeats", get(config::heartbeats::handler))
            .route("/containers/logs", get(containers::logs::handler))
            .route("/containers/list", get(containers::list::handler))
            .route("/system/bootstrap", post(system::bootstrap::handler))
            .route("/system/logs", get(system::logs::handler))
            .route("/system/stats", get(system::stats::handler))
            .with_state(state),
    )
}
