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
    state.services.workload.restart_workload(request.id).await?;
    Ok(Json(()))
}
