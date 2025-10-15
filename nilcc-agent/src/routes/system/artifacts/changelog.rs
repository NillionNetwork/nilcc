use crate::{
    repositories::changelog::{ChangelogEntryOperation, ChangelogEntryState},
    routes::AppState,
};
use axum::{Json, extract::State, response::IntoResponse};
use axum::{http::StatusCode, response::Response};
use nilcc_agent_models::{
    errors::RequestHandlerError,
    system::{
        ArtifactChangelogEntry, ArtifactChangelogEntryOperation, ArtifactChangelogEntryState, ArtifactChangelogResponse,
    },
};
use tracing::error;

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<ArtifactChangelogResponse>, Response> {
    let entries = state.services.upgrade.artifacts_changelog().await.map_err(|e| {
        error!("Failed to get artifacts changelog: {e:#}");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(RequestHandlerError::internal())).into_response()
    })?;

    let entries = entries
        .into_iter()
        .map(|entry| {
            let operation = match entry.entry.operation {
                ChangelogEntryOperation::Install => ArtifactChangelogEntryOperation::Install,
                ChangelogEntryOperation::Uninstall => ArtifactChangelogEntryOperation::Uninstall,
            };
            let state = match entry.entry.state {
                ChangelogEntryState::Pending => ArtifactChangelogEntryState::Pending,
                ChangelogEntryState::Success => ArtifactChangelogEntryState::Success,
                ChangelogEntryState::Failure => {
                    ArtifactChangelogEntryState::Failure { error: entry.entry.details.unwrap_or_default() }
                }
            };
            ArtifactChangelogEntry {
                version: entry.entry.version,
                operation,
                state,
                created_at: entry.created_at,
                updated_at: entry.updated_at,
            }
        })
        .collect();
    Ok(Json(ArtifactChangelogResponse { entries }))
}
