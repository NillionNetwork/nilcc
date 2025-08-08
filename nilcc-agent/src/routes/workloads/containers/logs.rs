use crate::{
    clients::cvm_agent::CvmAgentRequestError,
    routes::{workloads::containers::CvmAgentHandlerError, AppState, Json, Query},
};
use axum::extract::{Path, State};
use cvm_agent_models::logs::{ContainerLogsRequest, ContainerLogsResponse};
use reqwest::StatusCode;
use uuid::Uuid;

pub(crate) async fn handler(
    state: State<AppState>,
    path: Path<Uuid>,
    request: Query<ContainerLogsRequest>,
) -> Result<Json<ContainerLogsResponse>, CvmAgentHandlerError> {
    let port = state.services.workload.cvm_agent_port(path.0).await?;
    let result = state.clients.cvm_agent.container_logs(port, &request.0).await;
    match result {
        Ok(response) => Ok(Json(response)),
        Err(CvmAgentRequestError::Http(e)) if e.status() == Some(StatusCode::NOT_FOUND) => {
            Err(CvmAgentHandlerError::ContainerNotFound)
        }
        Err(e) => Err(CvmAgentHandlerError::Internal(e.to_string())),
    }
}
