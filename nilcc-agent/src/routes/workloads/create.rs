use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::CreateWorkloadError,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_valid::Valid;
use cvm_agent_models::bootstrap::CADDY_ACME_EAB_KEY_ID;
use nilcc_agent_models::workloads::create::{CreateWorkloadRequest, CreateWorkloadResponse};
use strum::EnumDiscriminants;
use tracing::error;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Valid<Json<CreateWorkloadRequest>>,
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
    if request.docker_compose.contains(CADDY_ACME_EAB_KEY_ID) {
        return Err(HandlerError::CaddyAcmeKey);
    }

    let id = request.id;
    state.services.workload.create_workload(request.0 .0).await?;
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

    #[error("domain is already managed by another workload")]
    DomainExists,

    #[error("{0} can't be higher than {1}")]
    ResourceLimit(&'static str, u32),

    #[error("{CADDY_ACME_EAB_KEY_ID} is a reserved environment variable")]
    CaddyAcmeKey,
}

impl From<CreateWorkloadError> for HandlerError {
    fn from(e: CreateWorkloadError) -> Self {
        match e {
            CreateWorkloadError::InsufficientResources(e) => Self::InsufficientResources(e),
            CreateWorkloadError::Internal(e) => Self::Internal(e),
            CreateWorkloadError::AlreadyExists => Self::AlreadyExists,
            CreateWorkloadError::DomainExists => Self::DomainExists,
        }
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        let discriminant = HandlerErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::InsufficientResources(_) => (StatusCode::PRECONDITION_FAILED, self.to_string()),
            Self::AlreadyExists | Self::DomainExists => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::Internal(e) => {
                error!("Failed to create workload: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
            }
            Self::ResourceLimit(..) | Self::CaddyAcmeKey => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
