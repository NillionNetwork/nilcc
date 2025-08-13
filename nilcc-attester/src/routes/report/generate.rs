use crate::{config::VmType, report::ReportData, routes::AppState};
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use sev::firmware::guest::AttestationReport;
use tracing::{error, info};

#[derive(Deserialize)]
pub(crate) struct Request {
    #[serde(with = "hex::serde")]
    nonce: ReportData,
}

#[derive(Serialize)]
pub(crate) struct Response {
    report: AttestationReport,
    gpu_token: Option<String>,
    environment: EnvironmentSpec,
}

#[derive(Serialize)]
pub(crate) struct EnvironmentSpec {
    nilcc_version: String,
    vm_type: VmType,
    cpu_count: usize,
}

pub(crate) async fn handler(state: State<AppState>, request: Json<Request>) -> Result<Json<Response>, StatusCode> {
    let AppState { nilcc_version, vm_type, cpu_count, hardware_reporter } = state.0;
    let nonce = request.nonce;
    let hex_nonce = hex::encode(nonce);
    info!("Generating hardware report using nonce {hex_nonce}");
    let report = match hardware_reporter.hardware_report(nonce) {
        Ok(report) => report,
        Err(e) => {
            error!("Failed to generate hardware report: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    // let gpu_token = match &vm_type {
    //     VmType::Cpu => None,
    //     VmType::Gpu => {
    //         info!("Generating GPU report using nonce {hex_nonce}");
    //         match hardware_reporter.gpu_report(&hex_nonce).await {
    //             Ok(token) => Some(token),
    //             Err(e) => {
    //                 error!("Failed to generate GPU attestation: {e:#}");
    //                 return Err(StatusCode::INTERNAL_SERVER_ERROR);
    //             }
    //         }
    //     }
    // };
    let gpu_token = None;
    let environment = EnvironmentSpec { nilcc_version, vm_type, cpu_count };
    Ok(Json(Response { report, environment, gpu_token }))
}
