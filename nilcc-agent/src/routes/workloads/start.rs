use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use nilcc_agent_models::workloads::start::StartWorkloadRequest;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<StartWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    state.services.workload.start_workload(request.id).await?;
    Ok(Json(()))
}
