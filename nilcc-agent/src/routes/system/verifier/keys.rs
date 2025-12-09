use crate::routes::{AppState, Json};
use axum::extract::State;
use nilcc_agent_models::system::VerifierKey;

pub(crate) async fn handler(state: State<AppState>) -> Json<Vec<VerifierKey>> {
    let keys = state.verifier_keys.public_keys().into_iter().map(|k| VerifierKey { public_key: k.into() }).collect();
    Json(keys)
}
