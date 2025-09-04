use crate::{
    routes::{AppState, Json},
    services::upgrade::UpgradeError,
};
use axum::extract::State;
use nilcc_agent_models::system::UpgradeRequest;

pub(crate) async fn handler(state: State<AppState>, request: Json<UpgradeRequest>) -> Result<Json<()>, UpgradeError> {
    let UpgradeRequest { version } = request.0;
    state.services.upgrade.upgrade_agent(version).await?;
    Ok(Json(()))
}
