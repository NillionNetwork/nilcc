use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use nilcc_agent_models::workloads::restart::RestartWorkloadRequest;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<RestartWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    let RestartWorkloadRequest { id, env_vars } = request.0;
    state.services.workload.restart_workload(id, env_vars).await?;
    Ok(Json(()))
}
