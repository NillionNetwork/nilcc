use crate::routes::SharedState;
use axum::Json;
use cvm_agent_models::health::HealthResponse;

use super::SystemState;

pub(crate) async fn handler(state: SharedState) -> Json<HealthResponse> {
    let (https, bootstrapped) = match &*state.system_state.lock().unwrap() {
        SystemState::WaitingBootstrap => (false, false),
        SystemState::Starting => (false, true),
        SystemState::Ready => (true, true),
    };
    let response = HealthResponse { https, bootstrapped };
    Json(response)
}
