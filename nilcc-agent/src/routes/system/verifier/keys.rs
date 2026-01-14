use crate::routes::{AppState, Json};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use nilcc_agent_models::{errors::RequestHandlerError, system::VerifierKey};
use std::collections::HashSet;

pub(crate) async fn handler(state: State<AppState>) -> Result<Json<Vec<VerifierKey>>, Response> {
    let workloads = state
        .services
        .workload
        .list_workloads()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(RequestHandlerError::internal())).into_response())?;
    let active_keys: HashSet<_> =
        workloads.into_iter().flat_map(|w| w.heartbeat.and_then(|h| h.wallet_public_key)).collect();
    let keys = state
        .verifier_keys
        .public_keys()
        .into_iter()
        .map(|k| {
            let public_key = k.public_uncompressed.into();
            let active = active_keys.contains(k.public.as_slice());
            VerifierKey { public_key, active }
        })
        .collect();
    Ok(Json(keys))
}
