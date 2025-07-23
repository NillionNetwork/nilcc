use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use nilcc_agent_models::workloads::stop::StopWorkloadRequest;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<StopWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    state.services.workload.stop_workload(request.id).await?;
    Ok(Json(()))
}
