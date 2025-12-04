use crate::certs::DefaultCertificateFetcher;
use crate::verify::ReportVerifier;
use axum::Router;
use axum::routing::post;
use axum::{Json, http::StatusCode, response::IntoResponse};
use convert_case::{Case, Casing};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};

mod v1;

pub(crate) fn build_router(cert_cache: PathBuf, artifacts_path: PathBuf) -> anyhow::Result<Router> {
    let cert_fetcher = Arc::new(DefaultCertificateFetcher::new(cert_cache)?);
    let report_verifier = ReportVerifier::new(cert_fetcher);
    let state = VerifyState { report_verifier, artifacts_path };
    let router = Router::new().nest(
        "/v1",
        Router::new()
            .route("/attestations/verify", post(v1::attestations::verify::handler))
            .route("/attestations/verify-amd", post(v1::attestations::verify_amd::handler))
            .with_state(state),
    );
    Ok(router)
}

#[derive(Clone)]
pub(crate) struct VerifyState {
    pub(crate) report_verifier: ReportVerifier,
    pub(crate) artifacts_path: PathBuf,
}

/// An error when handling a request.
#[derive(Clone, Debug)]
pub(crate) struct RequestHandlerError {
    status_code: StatusCode,
    message: String,
    error_code: String,
}

impl RequestHandlerError {
    pub(crate) fn new(status_code: StatusCode, message: impl Into<String>, error_code: impl AsRef<str>) -> Self {
        let error_code = error_code.as_ref().to_case(Case::UpperSnake);
        Self { status_code, message: message.into(), error_code }
    }

    pub(crate) fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error", "INTERNAL")
    }
}

impl IntoResponse for RequestHandlerError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct Inner {
            message: String,
            error_code: String,
        }

        let Self { status_code, message, error_code } = self;
        let inner = Inner { message, error_code };
        (status_code, Json(inner)).into_response()
    }
}
