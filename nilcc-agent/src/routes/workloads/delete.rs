use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::{WorkloadLookupError, WorkloadLookupErrorDiscriminants},
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

pub(crate) struct HandlerError(WorkloadLookupError);

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let discriminant = WorkloadLookupErrorDiscriminants::from(&self.0);
        let (code, message) = match self.0 {
            WorkloadLookupError::Database(e) => {
                error!("Failed to run queries: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            WorkloadLookupError::WorkloadNotFound => (StatusCode::NOT_FOUND, self.0.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
