use crate::{
    routes::{AppState, Json},
    services::workload::CreateWorkloadError,
};
use axum::extract::State;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct DeleteWorkloadRequest {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<DeleteWorkloadRequest>,
) -> Result<Json<()>, CreateWorkloadError> {
    state.services.workload.delete_workload(request.id).await;
    Ok(Json(()))
}
