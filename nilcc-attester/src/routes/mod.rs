use crate::{config::VmType, report::HardwareReporter};
use axum::{Router, routing::get};
use std::sync::Arc;

pub(crate) mod health;
pub(crate) mod v1;
pub(crate) mod v2;

/// Build a router for the entire service.
pub fn build_router(state: AppState) -> Router {
    let router_v1 = Router::new().route("/about", get(v1::about::handler)).route("/report", get(v1::report::handler));
    let router_v2 = Router::new().route("/report", get(v2::report::handler));
    Router::new()
        .route("/health", get(health::handler))
        .nest("/api/v1", router_v1)
        .nest("/api/v2", router_v2)
        .with_state(state)
}

#[derive(Clone)]
pub struct AppState {
    pub nilcc_version: String,
    pub vm_type: VmType,
    pub cpu_count: usize,
    pub reporter: Arc<HardwareReporter>,
}
