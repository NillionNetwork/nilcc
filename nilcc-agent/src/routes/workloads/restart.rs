use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RestartWorkloadRequest {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<RestartWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    state.services.workload.restart_workload(request.id).await?;
    Ok(Json(()))
}
