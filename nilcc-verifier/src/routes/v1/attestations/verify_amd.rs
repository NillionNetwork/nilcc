use crate::error::{ErrorCode, ValidateError};
use crate::routes::{RequestHandlerError, VerifyState};
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use sev::firmware::guest::AttestationReport;
use sev::parser::ByteParser;
use tracing::warn;

#[serde_as]
#[derive(Deserialize)]
pub(crate) struct VerifyRequest {
    #[serde_as(as = "Hex")]
    report: Vec<u8>,
}

#[serde_as]
#[derive(Serialize)]
pub(crate) struct VerifyResponse {
    #[serde_as(as = "Hex")]
    chip_id: [u8; 64],
}

pub(crate) async fn handler(
    state: State<VerifyState>,
    request: Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, RequestHandlerError> {
    let VerifyRequest { report } = request.0;
    let report = AttestationReport::from_bytes(&report).map_err(|_| {
        RequestHandlerError::new(StatusCode::BAD_REQUEST, "malformed attestation report", "MALFORMED_REPORT")
    })?;
    // Verify the report and pass in its own measurement hash since we don't care about its value.
    state.report_verifier.verify_report(&report, &report.measurement).await.map_err(|e| {
        warn!("Failed to verify report: {e:#}");
        let error_code = ErrorCode::from(ValidateError::VerifyReports(e));
        RequestHandlerError::new(
            StatusCode::PRECONDITION_FAILED,
            "report verification failed",
            format!("{error_code:?}"),
        )
    })?;

    let response = VerifyResponse { chip_id: report.chip_id };
    Ok(Json(response))
}
