use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use bollard::Docker;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

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
    pub external_files: PathBuf,
    pub caddy_config: PathBuf,
    pub version: String,
    pub vm_type: String,
    pub iso_mount: PathBuf,
}

pub struct AppState {
    pub docker: Docker,
    pub context: BootstrapContext,
    pub system_state: Arc<Mutex<SystemState>>,
    pub log_path: PathBuf,
}

pub(crate) type SharedState = State<Arc<AppState>>;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new().nest(
        "/api/v1",
        Router::new()
            .route("/health", get(health::handler))
            .route("/containers/logs", get(containers::logs::handler))
            .route("/containers/list", get(containers::list::handler))
            .route("/system/bootstrap", post(system::bootstrap::handler))
            .route("/system/logs", get(system::logs::handler))
            .route("/system/stats", get(system::stats::handler))
            .with_state(state),
    )
}
