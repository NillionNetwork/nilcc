use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use strum::EnumDiscriminants;
use tracing::error;

use crate::routes::{Json, RequestHandlerError};
use crate::services::workload::WorkloadLookupError;

pub(crate) mod list;
pub(crate) mod logs;

#[derive(EnumDiscriminants)]
pub(crate) enum CvmAgentHandlerError {
    Internal(String),
    WorkloadNotFound,
}

impl From<WorkloadLookupError> for CvmAgentHandlerError {
    fn from(e: WorkloadLookupError) -> Self {
        match e {
            WorkloadLookupError::WorkloadNotFound => Self::WorkloadNotFound,
            WorkloadLookupError::Database(e) => Self::Internal(e.to_string()),
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
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
