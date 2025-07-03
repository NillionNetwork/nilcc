use crate::routes::{AppState, Json};
use axum::extract::State;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct DeleteWorkloadRequest {
    pub(crate) id: Uuid,
}

pub(crate) async fn handler(state: State<AppState>, request: Json<DeleteWorkloadRequest>) -> Json<()> {
    state.services.workload.delete_workload(request.id).await;
    Json(())
}
