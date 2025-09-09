use crate::{
    routes::{AppState, Json},
    services::upgrade::CleanupError,
};
use axum::extract::State;
use nilcc_agent_models::system::ArtifactsCleanupResponse;

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<ArtifactsCleanupResponse>, CleanupError> {
    let versions_deleted = state.services.upgrade.cleanup_artifacts().await?;
    let response = ArtifactsCleanupResponse { versions_deleted };
    Ok(Json(response))
}
