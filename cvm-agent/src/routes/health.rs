use super::SystemState;
use crate::routes::SharedState;
use axum::Json;
use cvm_agent_models::health::HealthResponse;

pub(crate) async fn handler(state: SharedState) -> Json<HealthResponse> {
    let (https, bootstrapped) = match &*state.system_state.lock().await {
        SystemState::WaitingBootstrap => (false, false),
        SystemState::Starting => (false, true),
        SystemState::Ready => (true, true),
    };

    let last_event = state.context.event_holder.get();
    let response = HealthResponse { https, bootstrapped, last_event };
    Json(response)
}
