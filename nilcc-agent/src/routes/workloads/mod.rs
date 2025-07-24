use crate::routes::{Json, RequestHandlerError};
use crate::services::workload::{WorkloadLookupError, WorkloadLookupErrorDiscriminants};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::error;

pub(crate) mod containers;
pub(crate) mod create;
pub(crate) mod delete;
pub(crate) mod list;
pub(crate) mod restart;
pub(crate) mod start;
pub(crate) mod stop;

impl IntoResponse for WorkloadLookupError {
    fn into_response(self) -> Response {
        let discriminant = WorkloadLookupErrorDiscriminants::from(&self);
        let (code, message) = match self {
            WorkloadLookupError::Database(e) => {
                error!("Failed to run queries: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            WorkloadLookupError::WorkloadNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            WorkloadLookupError::Internal(e) => {
                error!("Failed to process request: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
