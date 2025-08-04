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
use tokio::process::Child;

pub(crate) mod containers;
pub(crate) mod health;
pub(crate) mod system;

#[derive(Default)]
pub enum SystemState {
    #[default]
    Pending,
    Running(Child),
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
    pub system_state: Mutex<SystemState>,
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
            .with_state(state),
    )
}
