use crate::routes::{workloads::containers::CvmAgentHandlerError, AppState, Json};
use axum::extract::{Path, Query, State};
use cvm_agent_models::logs::{SystemLogsRequest, SystemLogsResponse};
use uuid::Uuid;

pub(crate) async fn handler(
    state: State<AppState>,
    path: Path<Uuid>,
    request: Query<SystemLogsRequest>,
) -> Result<Json<SystemLogsResponse>, CvmAgentHandlerError> {
    let port = state.services.workload.cvm_agent_port(path.0).await?;
    let result = state.clients.cvm_agent.system_logs(port, &request.0).await;
    match result {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err(CvmAgentHandlerError::Internal(e.to_string())),
    }
}
