use crate::{
    report::{request_hardware_report, ReportData},
    routes::AppState,
};
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
    nilcc_version: Option<String>,
    cpu_count: usize,
}

pub(crate) async fn handler(state: State<AppState>, request: Json<Request>) -> Result<Json<Response>, StatusCode> {
    let state = state.0;
    let data = request.nonce;
    info!("Generating report using nonce {}", hex::encode(data));
    let report = match request_hardware_report(data) {
        Ok(report) => report,
        Err(e) => {
            error!("Failed to generate report: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(Response { report, nilcc_version: state.nilcc_version, cpu_count: state.cpu_count }))
}
