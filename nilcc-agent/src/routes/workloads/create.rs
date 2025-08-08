use crate::{
    routes::{AppState, Json, RequestHandlerError},
    services::workload::CreateWorkloadError,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use cvm_agent_models::bootstrap::CADDY_ACME_EAB_KEY_ID;
use docker_compose_types::Compose;
use nilcc_agent_models::workloads::create::{CreateWorkloadRequest, CreateWorkloadResponse};
use std::iter;
use strum::EnumDiscriminants;
use tracing::error;

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
    if request.docker_compose.contains(CADDY_ACME_EAB_KEY_ID) {
        return Err(HandlerError::CaddyAcmeKey);
    }

    validate_compose_file(&request)?;

    let id = request.id;
    state.services.workload.create_workload(request.0).await?;
    Ok(Json(CreateWorkloadResponse { id }))
}

fn validate_compose_file(request: &CreateWorkloadRequest) -> Result<(), HandlerError> {
    let compose: Compose =
        serde_yaml::from_str(&request.docker_compose).map_err(HandlerError::MalformedDockerCompose)?;
    if compose.services.is_empty() {
        return Err(HandlerError::InvalidDockerCompose("no services defined".into()));
    }
    for (name, service) in &compose.services.0 {
        let service = service
            .as_ref()
            .ok_or_else(|| HandlerError::InvalidDockerCompose(format!("no body in service '{name}'")))?;
        let names = iter::once(name).chain(service.container_name.as_ref());
        for name in names {
            if name == &request.public_container_name {
                return Ok(());
            }
        }
    }
    Err(HandlerError::InvalidDockerCompose(format!(
        "container {} is not part of compose file",
        request.public_container_name
    )))
}

#[derive(Debug, thiserror::Error, EnumDiscriminants)]
pub(crate) enum HandlerError {
    #[error("not enough {0} avalable")]
    InsufficientResources(&'static str),

    #[error("internal: {0}")]
    Internal(String),

    #[error("workload already exists")]
    AlreadyExists,

    #[error("malformed docker compose: {0}")]
    MalformedDockerCompose(serde_yaml::Error),

    #[error("invalid docker compose: {0}")]
    InvalidDockerCompose(String),

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
            Self::AlreadyExists
            | Self::DomainExists
            | Self::MalformedDockerCompose(_)
            | Self::InvalidDockerCompose(_) => (StatusCode::BAD_REQUEST, self.to_string()),
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
