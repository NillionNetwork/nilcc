use crate::{
    monitors::{caddy::CaddyMonitor, compose::ComposeMonitor},
    routes::{SharedState, SystemState},
};
use axum::{http::StatusCode, Json};
use cvm_agent_models::bootstrap::BootstrapRequest;

pub(crate) async fn handler(state: SharedState, request: Json<BootstrapRequest>) -> StatusCode {
    let mut system_state = state.system_state.lock().unwrap();
    if !matches!(&*system_state, SystemState::WaitingBootstrap) {
        return StatusCode::OK;
    }
    let request = request.0;
    let ctx = state.context.clone();
    *system_state = SystemState::Starting;
    ComposeMonitor::spawn(ctx, request.acme, request.docker);
    CaddyMonitor::spawn(state.docker.clone(), state.system_state.clone());
    StatusCode::OK
}
