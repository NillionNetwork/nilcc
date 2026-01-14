use crate::routes::SharedState;
use axum::{Json, http::StatusCode};
use cvm_agent_models::config::HeartbeatConfigRequest;
use tracing::info;

pub(crate) async fn handler(state: SharedState, request: Json<HeartbeatConfigRequest>) -> StatusCode {
    let HeartbeatConfigRequest { interval } = request.0;
    match &mut *state.heartbeat_handle.lock().await {
        Some(handle) => {
            handle.set_interval(interval).await;
            info!("Changed heartbeat interval to {interval:?}");
            StatusCode::OK
        }
        None => StatusCode::PRECONDITION_FAILED,
    }
}
