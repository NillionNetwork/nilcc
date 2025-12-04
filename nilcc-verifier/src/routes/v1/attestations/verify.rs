use crate::routes::{RequestHandlerError, VerifyState};
use attestation_verification::{ErrorCode, MeasurementGenerator, ValidateError, VmType};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use nilcc_artifacts::downloader::ArtifactsDownloader;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use sev::firmware::guest::AttestationReport;
use sev::parser::ByteParser;
use tracing::{error, warn};

#[serde_as]
#[derive(Deserialize)]
pub(crate) struct VerifyRequest {
    #[serde_as(as = "Hex")]
    report: Vec<u8>,

    #[serde_as(as = "Hex")]
    docker_compose_hash: [u8; 32],

    nilcc_version: String,
    vcpus: u32,
    vm_type: VmType,
}

#[derive(Serialize)]
pub(crate) struct VerifyResponse {}

pub(crate) async fn handler(
    state: State<VerifyState>,
    request: Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, RequestHandlerError> {
    let VerifyRequest { report, docker_compose_hash, nilcc_version, vcpus, vm_type } = request.0;
    let vm_type = vm_type.into();
    let report = AttestationReport::from_bytes(&report).map_err(|_| {
        RequestHandlerError::new(StatusCode::BAD_REQUEST, "malformed attestation report", "MALFORMED_REPORT")
    })?;
    let artifacts_path = state.artifacts_path.join(&nilcc_version);
    let artifacts = ArtifactsDownloader::new(nilcc_version.clone(), vec![vm_type])
        .without_disk_images()
        .without_artifact_overwrite()
        .download(&artifacts_path)
        .await
        .map_err(|e| {
            warn!("Failed to download artifact version '{nilcc_version}': {e:#}");
            RequestHandlerError::internal()
        })?;
    let measurement_hash =
        MeasurementGenerator::new(docker_compose_hash, vcpus, vm_type, &artifacts.metadata, &artifacts_path)
            .generate()
            .map_err(|e| {
                error!("Failed to generate measurement hash: {e:#}");
                RequestHandlerError::internal()
            })?;
    state.report_verifier.verify_report(&report, &measurement_hash).await.map_err(|e| {
        warn!("Failed to verify report: {e:#}");
        let error_code = ErrorCode::from(ValidateError::VerifyReports(e));
        RequestHandlerError::new(
            StatusCode::PRECONDITION_FAILED,
            "report verification failed",
            format!("{error_code:?}"),
        )
    })?;

    let response = VerifyResponse {};
    Ok(Json(response))
}
