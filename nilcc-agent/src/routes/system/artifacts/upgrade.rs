use crate::{
    routes::{AppState, Json},
    services::upgrade::{UpgradeError, UpgradeErrorDiscriminants},
};
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use nilcc_agent_models::{errors::RequestHandlerError, system::UpgradeArtifactsRequest};
use reqwest::StatusCode;

pub(crate) async fn handler(
    state: State<AppState>,
    request: Json<UpgradeArtifactsRequest>,
) -> Result<Json<()>, UpgradeError> {
    let UpgradeArtifactsRequest { version } = request.0;
    let path = state.cvm_artifacts_path.join(&version);
    state.services.upgrade.upgrade_artifacts(version, state.vm_types.clone(), path).await?;
    Ok(Json(()))
}

impl IntoResponse for UpgradeError {
    fn into_response(self) -> Response {
        let discriminant = UpgradeErrorDiscriminants::from(&self);
        let (code, message) = match self {
            Self::InvalidVersion => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::ActiveUpgrade(_) => (StatusCode::PRECONDITION_FAILED, self.to_string()),
        };
        let response = RequestHandlerError::new(message, format!("{discriminant:?}"));
        (code, Json(response)).into_response()
    }
}
