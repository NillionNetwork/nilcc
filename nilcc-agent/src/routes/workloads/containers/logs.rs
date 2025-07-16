use crate::routes::{workloads::containers::CvmAgentHandlerError, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use cvm_agent_models::logs::{ContainersLogsRequest, ContainersLogsResponse};
use uuid::Uuid;

pub(crate) async fn handler(
    state: State<AppState>,
    path: Path<Uuid>,
    request: Query<ContainersLogsRequest>,
) -> Result<Json<ContainersLogsResponse>, CvmAgentHandlerError> {
    let port = state.services.workload.cvm_agent_port(path.0).await?;
    state
        .clients
        .cvm_agent
        .logs(port, &request.0)
        .await
        .map(Json)
        .map_err(|e| CvmAgentHandlerError::Internal(format!("{e:#}")))
}
