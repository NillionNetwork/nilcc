use crate::{
    config::VmType,
    report::{GpuReportData, HardwareReportData},
    routes::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use sev::firmware::guest::AttestationReport;
use tracing::error;

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Request {
    #[serde(with = "hex::serde")]
    nonce: HardwareReportData,

    #[serde_as(as = "Option<Hex>")]
    gpu_nonce: Option<GpuReportData>,
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
    let Request { nonce, gpu_nonce } = request.0;
    let report = match hardware_reporter.hardware_report(nonce) {
        Ok(report) => report,
        Err(e) => {
            error!("Failed to generate hardware report: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let gpu_token = match (&vm_type, gpu_nonce) {
        (VmType::Cpu, _) => None,
        (VmType::Gpu, Some(nonce)) => match hardware_reporter.gpu_report(nonce).await {
            Ok(token) => Some(token),
            Err(e) => {
                error!("Failed to generate GPU attestation: {e:#}");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        },
        (VmType::Gpu, None) => None,
    };
    let environment = EnvironmentSpec { nilcc_version, vm_type, cpu_count };
    Ok(Json(Response { report, environment, gpu_token }))
}
