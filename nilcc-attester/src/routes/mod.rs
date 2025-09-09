use crate::{config::VmType, report::HardwareReporter};
use axum::{routing::get, Router};
use std::sync::Arc;

pub(crate) mod about;
pub(crate) mod health;
pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router(state: AppState) -> Router {
    Router::new().route("/health", get(health::handler)).nest(
        "/api/v1",
        Router::new().route("/about", get(about::handler)).route("/report", get(report::handler).with_state(state)),
    )
}

#[derive(Clone)]
pub struct AppState {
    pub nilcc_version: String,
    pub vm_type: VmType,
    pub cpu_count: usize,
    pub reporter: Arc<HardwareReporter>,
}
