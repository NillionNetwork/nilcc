use crate::{config::VmType, report::Reports, routes::AppState};
use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use serde_with::hex::Hex;
use serde_with::serde_as;
use std::sync::Arc;

#[serde_as]
#[derive(Serialize)]
pub(crate) struct Response {
    report: Arc<attestation_report::v2::AttestationReport>,
    #[serde_as(as = "Hex")]
    raw_report: Vec<u8>,
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
    let Reports { attestation, raw_attestation, gpu_token, .. } = reporter.reports().await;
    let environment = EnvironmentSpec { nilcc_version, vm_type, cpu_count };
    Ok(Json(Response { report: attestation, raw_report: raw_attestation, environment, gpu_token }))
}
