use std::{path::PathBuf, sync::Arc};

use axum::{
    routing::{get, post},
    Router,
};

use crate::{config::VmType, report::HardwareReporter};

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
    pub(crate) vm_type: VmType,
    pub(crate) cpu_count: usize,
    pub(crate) hardware_reporter: Arc<HardwareReporter>,
}

impl AppState {
    pub fn new(nilcc_version: String, vm_type: VmType, gpu_attester_path: PathBuf) -> Self {
        let cpu_count = num_cpus::get();
        let hardware_reporter = Arc::new(HardwareReporter::new(gpu_attester_path));
        Self { nilcc_version, vm_type, cpu_count, hardware_reporter }
    }
}
