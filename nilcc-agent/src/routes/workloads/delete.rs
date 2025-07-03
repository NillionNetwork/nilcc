use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::{DeleteWorkloadError, DeleteWorkloadErrorDiscriminants},
};
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::error;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct DeleteWorkloadRequest {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<DeleteWorkloadRequest>,
) -> Result<Json<()>, HandlerError> {
    state.services.workload.delete_workload(request.id).await.map_err(HandlerError)?;
    Ok(Json(()))
}

pub(crate) struct HandlerError(DeleteWorkloadError);

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let discriminant = DeleteWorkloadErrorDiscriminants::from(&self.0);
        let (code, message) = match self.0 {
            DeleteWorkloadError::Database(e) => {
                error!("Failed to run queries: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            DeleteWorkloadError::WorkloadNotFound => (StatusCode::PRECONDITION_FAILED, self.0.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
