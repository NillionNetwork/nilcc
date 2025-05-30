use anyhow::anyhow;
use axum::{
    routing::{get, post},
    Router,
};
use std::env;

pub(crate) mod health;
pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router() -> anyhow::Result<Router> {
    let state = AppState::new()?;
    let router = Router::new()
        .route("/health", get(health::handler))
        .nest("/api/v1", Router::new().route("/report/generate", post(report::generate::handler).with_state(state)));
    Ok(router)
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) nilcc_version: String,
    pub(crate) vm_type: String,
    pub(crate) cpu_count: usize,
}

impl AppState {
    fn new() -> anyhow::Result<Self> {
        let nilcc_version = env::var("NILCC_VERSION").map_err(|_| anyhow!("NILCC_VERSION not set"))?;
        let vm_type = env::var("NILCC_VM_TYPE").map_err(|_| anyhow!("NILCC_VM_TYPE not set"))?;
        let cpu_count = num_cpus::get();
        Ok(Self { nilcc_version, vm_type, cpu_count })
    }
}
