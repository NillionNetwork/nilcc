use crate::{config::VmType, report::Reports, routes::AppState};
use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub(crate) struct Response {
    report: Arc<attestation_report::v1::AttestationReport>,
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
    let AppState { nilcc_version, vm_type, cpu_count, reporter } = state.0;
    let Reports { attestation_v1, gpu_token, .. } = reporter.reports().await;
    let environment = EnvironmentSpec { nilcc_version, vm_type, cpu_count };
    Ok(Json(Response { report: attestation_v1, environment, gpu_token }))
}
