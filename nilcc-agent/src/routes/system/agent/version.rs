use crate::{
    routes::AppState,
    services::upgrade::{UpgradeMetadata, UpgradeState},
};
use axum::response::Response;
use axum::{extract::State, Json};
use nilcc_agent_models::system::{LastUpgrade, VersionResponse};

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<VersionResponse>, Response> {
    let version = state.services.upgrade.agent_version();
    let last_upgrade = match state.services.upgrade.agent_upgrade_state().await {
        UpgradeState::None => None,
        UpgradeState::Upgrading { metadata } => {
            let UpgradeMetadata { version, started_at, .. } = metadata;
            Some(LastUpgrade { version, started_at, state: nilcc_agent_models::system::UpgradeState::InProgress })
        }
        UpgradeState::Done { metadata, finished_at, error } => {
            let UpgradeMetadata { version, started_at, .. } = metadata;
            let state = match error {
                Some(error) => nilcc_agent_models::system::UpgradeState::Error { error, finished_at },
                None => nilcc_agent_models::system::UpgradeState::Success { finished_at },
            };
            Some(LastUpgrade { version, started_at, state })
        }
    };
    Ok(Json(VersionResponse { version, last_upgrade }))
}
