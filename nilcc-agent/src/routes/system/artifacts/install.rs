use crate::{
    routes::{AppState, Json},
    services::upgrade::UpgradeError,
};
use axum::extract::State;
use nilcc_agent_models::system::InstallArtifactVersionRequest;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<InstallArtifactVersionRequest>,
) -> Result<Json<()>, UpgradeError> {
    let InstallArtifactVersionRequest { version } = request.0;
    state.services.upgrade.install_artifacts(version).await?;
    Ok(Json(()))
}
