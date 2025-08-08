use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use nilcc_agent_models::workloads::list::WorkloadSummary;

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<Vec<WorkloadSummary>>, WorkloadLookupError> {
    let workloads = state.services.workload.list_workloads().await?;
    let workloads =
        workloads.into_iter().map(|w| WorkloadSummary { id: w.id, enabled: w.enabled, domain: w.domain }).collect();
    Ok(Json(workloads))
}
