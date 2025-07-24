use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use nilcc_agent_models::workloads::delete::DeleteWorkloadRequest;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<DeleteWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    state.services.workload.delete_workload(request.id).await?;
    Ok(Json(()))
}
