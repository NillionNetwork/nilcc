use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::{CreateWorkloadError, CreateWorkloadErrorDiscriminants},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct CreateWorkloadRequest {
    pub(crate) id: Uuid,
    pub(crate) docker_compose: String,
    pub(crate) env_vars: HashMap<String, String>,
    pub(crate) public_container_name: String,
    pub(crate) public_container_port: u16,
    pub(crate) memory_mb: u32,
    pub(crate) cpus: u32,
    pub(crate) gpus: u16,
    pub(crate) disk_space_gb: u32,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CreateWorkloadResponse {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<CreateWorkloadRequest>,
) -> Result<Json<CreateWorkloadResponse>, CreateWorkloadError> {
    let id = request.id;
    state.services.workload.create_workload(request.0).await?;
    Ok(Json(CreateWorkloadResponse { id }))
}

impl IntoResponse for CreateWorkloadError {
    fn into_response(self) -> Response {
        let discriminant = CreateWorkloadErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::InsufficientResources(_) => (StatusCode::PRECONDITION_FAILED, self.to_string()),
            Self::Internal(e) => {
                error!("Failed to create workload: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            Self::AlreadyExists => (StatusCode::BAD_REQUEST, "workload already exists".into()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
