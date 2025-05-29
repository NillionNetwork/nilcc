use axum::{
    routing::{get, post},
    Router,
};
use std::env;

pub(crate) mod health;
pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router() -> Router {
    let state = AppState::new();
    Router::new()
        .route("/health", get(health::handler))
        .nest("/api/v1", Router::new().route("/report/generate", post(report::generate::handler).with_state(state)))
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) nilcc_version: Option<String>,
    pub(crate) cpu_count: usize,
}

impl AppState {
    fn new() -> Self {
        let nilcc_version = env::var("NILCC_VERSION").ok();
        let cpu_count = num_cpus::get();
        Self { nilcc_version, cpu_count }
    }
}
