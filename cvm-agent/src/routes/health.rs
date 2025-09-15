use super::SystemState;
use crate::routes::SharedState;
use axum::Json;
use cvm_agent_models::health::{HealthResponse, LastError};

pub(crate) async fn handler(state: SharedState) -> Json<HealthResponse> {
    let (https, bootstrapped) = match &*state.system_state.lock().unwrap() {
        SystemState::WaitingBootstrap => (false, false),
        SystemState::Starting => (false, true),
        SystemState::Ready => (true, true),
    };

    let last_event = state.context.event_holder.get();
    let last_error =
        last_event.clone().map(|e| LastError { error_id: e.id, message: e.message, failed_at: e.timestamp });
    let response = HealthResponse { https, bootstrapped, last_error, last_event };
    Json(response)
}
