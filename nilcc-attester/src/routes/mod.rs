use axum::{
    routing::{get, post},
    Router,
};

pub(crate) mod health;
pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::handler))
        .nest("/api/v1", Router::new().route("/report/generate", post(report::generate::handler).with_state(state)))
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) nilcc_version: String,
    pub(crate) vm_type: String,
    pub(crate) cpu_count: usize,
}

impl AppState {
    pub fn new(nilcc_version: String, vm_type: String) -> Self {
        let cpu_count = num_cpus::get();
        Self { nilcc_version, vm_type, cpu_count }
    }
}
