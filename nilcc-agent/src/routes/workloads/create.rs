use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::CreateWorkloadError,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::serde_as;
use std::collections::HashMap;
use strum::EnumDiscriminants;
use tracing::error;
use uuid::Uuid;
use validator::Validate;

#[serde_as]
#[derive(Clone, Debug, Deserialize, Validate)]
pub struct CreateWorkloadRequest {
    pub(crate) id: Uuid,
    pub(crate) docker_compose: String,
    #[serde(default)]
    pub(crate) env_vars: HashMap<String, String>,
    #[serde_as(deserialize_as = "HashMap<_, Base64>")]
    #[serde(default)]
    pub(crate) files: HashMap<String, Vec<u8>>,
    pub(crate) public_container_name: String,
    pub(crate) public_container_port: u16,
    #[validate(range(min = 512))]
    pub(crate) memory_mb: u32,
    #[validate(range(min = 1))]
    pub(crate) cpus: u32,
    pub(crate) gpus: u16,
    #[validate(range(min = 2))]
    pub(crate) disk_space_gb: u32,
    pub(crate) domain: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CreateWorkloadResponse {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<CreateWorkloadRequest>,
) -> Result<Json<CreateWorkloadResponse>, HandlerError> {
    let limits = &state.resource_limits;
    let checks = [
        (request.cpus, limits.cpus, "cpus"),
        (request.memory_mb, limits.memory_mb, "memory"),
        (request.disk_space_gb, limits.disk_space_gb, "disk space"),
    ];
    for (resource, limit, name) in checks {
        if resource > limit {
            return Err(HandlerError::ResourceLimit(name, limit));
        }
    }

    let id = request.id;
    state.services.workload.create_workload(request.0).await?;
    Ok(Json(CreateWorkloadResponse { id }))
}

#[derive(Debug, thiserror::Error, EnumDiscriminants)]
pub(crate) enum HandlerError {
    #[error("not enough {0} avalable")]
    InsufficientResources(&'static str),

    #[error("internal: {0}")]
    Internal(String),

    #[error("workload already exists")]
    AlreadyExists,

    #[error("{0} can't be higher than {1}")]
    ResourceLimit(&'static str, u32),
}

impl From<CreateWorkloadError> for HandlerError {
    fn from(e: CreateWorkloadError) -> Self {
        match e {
            CreateWorkloadError::InsufficientResources(e) => Self::InsufficientResources(e),
            CreateWorkloadError::Internal(e) => Self::Internal(e),
            CreateWorkloadError::AlreadyExists => Self::AlreadyExists,
        }
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let discriminant = HandlerErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::InsufficientResources(_) => (StatusCode::PRECONDITION_FAILED, self.to_string()),
            Self::AlreadyExists => (StatusCode::BAD_REQUEST, "workload already exists".into()),
            Self::Internal(e) => {
                error!("Failed to create workload: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            Self::ResourceLimit(..) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
