use crate::{
    routes::{AppState, Json},
    services::workload::WorkloadLookupError,
};
use axum::extract::State;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StartWorkloadRequest {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<StartWorkloadRequest>,
) -> Result<Json<()>, WorkloadLookupError> {
    state.services.workload.start_workload(request.id).await?;
    Ok(Json(()))
}
