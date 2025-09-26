use crate::routes::{AppState, Json, workloads::containers::CvmAgentHandlerError};
use axum::extract::{Path, State};
use cvm_agent_models::stats::SystemStatsResponse;
use uuid::Uuid;

pub(crate) async fn handler(
    state: State<AppState>,
    path: Path<Uuid>,
) -> Result<Json<SystemStatsResponse>, CvmAgentHandlerError> {
    let port = state.services.workload.cvm_agent_port(path.0).await?;
    let response = state.clients.cvm_agent.system_stats(port).await?;
    Ok(Json(response))
}
