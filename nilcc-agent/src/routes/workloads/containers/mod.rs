use crate::clients::cvm_agent::CvmAgentRequestError;
use crate::routes::{Json, RequestHandlerError};
use crate::services::workload::WorkloadLookupError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use strum::EnumDiscriminants;
use tracing::error;

pub(crate) mod list;
pub(crate) mod logs;

#[derive(EnumDiscriminants)]
pub(crate) enum CvmAgentHandlerError {
    Internal(String),
    WorkloadNotFound,
    ContainerNotFound,
    CvmAgent(&'static str),
}

impl From<CvmAgentRequestError> for CvmAgentHandlerError {
    fn from(e: CvmAgentRequestError) -> Self {
        use CvmAgentRequestError::*;
        match e {
            Http(e) if e.is_connect() => Self::CvmAgent("could not connect to cvm-agent"),
            Http(e) if e.is_timeout() => Self::CvmAgent("timed out waiting for cvm-agent"),
            Http(e) if e.is_request() => Self::CvmAgent("failed to send request to cvm-agent"),
            _ => Self::Internal(e.to_string()),
        }
    }
}

impl From<WorkloadLookupError> for CvmAgentHandlerError {
    fn from(e: WorkloadLookupError) -> Self {
        match e {
            WorkloadLookupError::WorkloadNotFound => Self::WorkloadNotFound,
            WorkloadLookupError::Database(e) => Self::Internal(e.to_string()),
            WorkloadLookupError::Internal(e) => Self::Internal(e.to_string()),
        }
    }
}

impl IntoResponse for CvmAgentHandlerError {
    fn into_response(self) -> Response {
        let discriminant = CvmAgentHandlerErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::Internal(e) => {
                error!("Failed to process request: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
            Self::WorkloadNotFound => (StatusCode::NOT_FOUND, "workload not found".into()),
            Self::ContainerNotFound => (StatusCode::NOT_FOUND, "container not found".into()),
            Self::CvmAgent(details) => (StatusCode::PRECONDITION_FAILED, details.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
