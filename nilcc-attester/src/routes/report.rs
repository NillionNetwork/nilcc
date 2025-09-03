use crate::{config::VmType, routes::AppState};
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use sev::firmware::guest::AttestationReport;
use std::sync::Arc;

#[derive(Serialize)]
pub(crate) struct Response {
    report: Arc<AttestationReport>,
    gpu_token: Option<String>,
    environment: EnvironmentSpec,
}

#[derive(Serialize)]
pub(crate) struct EnvironmentSpec {
    nilcc_version: String,
    vm_type: VmType,
    cpu_count: usize,
}

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<Response>, StatusCode> {
    let AppState { nilcc_version, vm_type, cpu_count, hardware_report, gpu_token } = state.0;
    let environment = EnvironmentSpec { nilcc_version, vm_type, cpu_count };
    Ok(Json(Response { report: hardware_report, environment, gpu_token }))
}
