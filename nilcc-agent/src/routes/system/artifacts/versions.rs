use crate::{
    routes::AppState,
    services::upgrade::{UpgradeMetadata, UpgradeState},
};
use axum::{extract::State, response::IntoResponse, Json};
use axum::{http::StatusCode, response::Response};
use nilcc_agent_models::{
    errors::RequestHandlerError,
    system::{ArtifactVersionsResponse, LastUpgrade},
};
use tracing::error;

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<ArtifactVersionsResponse>, Response> {
    let versions = state.services.upgrade.artifacts_versions().await.map_err(|e| {
        error!("Failed to get current artifacts version: {e:#}");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(RequestHandlerError::new("internal server error", "INTERNAL")))
            .into_response()
    })?;
    let last_upgrade = match state.services.upgrade.artifacts_upgrade_state().await {
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
    Ok(Json(ArtifactVersionsResponse { versions, last_upgrade }))
}
