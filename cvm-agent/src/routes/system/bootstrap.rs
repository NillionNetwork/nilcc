use crate::{
    heartbeat::{HeartbeatEmitter, HeartbeatEmitterArgs},
    monitors::{caddy::CaddyMonitor, compose::ComposeMonitor},
    routes::{SharedState, SystemState},
};
use axum::{Json, http::StatusCode};
use cvm_agent_models::bootstrap::BootstrapRequest;
use tracing::{error, info};

pub(crate) async fn handler(state: SharedState, request: Json<BootstrapRequest>) -> StatusCode {
    let mut system_state = state.system_state.lock().await;
    if !matches!(&*system_state, SystemState::WaitingBootstrap) {
        return StatusCode::OK;
    }
    let request = request.0;
    let ctx = state.context.clone();
    let event_holder = ctx.event_holder.clone();
    *system_state = SystemState::Starting;

    let caddy_status = CaddyMonitor::spawn(state.docker.clone(), state.system_state.clone(), event_holder);

    match (request.workload_id, request.heartbeat) {
        (Some(workload_id), Some(heartbeat)) => {
            let args = HeartbeatEmitterArgs {
                workload_id,
                workload_domain: request.domain.clone(),
                rpc_endpoint: heartbeat.rpc_endpoint,
                contract_address: heartbeat.contract_address,
                wallet_private_key: heartbeat.wallet_private_key,
                nilcc_version: ctx.version.clone(),
                docker_compose_hash: ctx.user_docker_compose_sha256,
                tick_interval: heartbeat.interval,
                measurement_hash_url: heartbeat.measurement_hash_url,
                cpu_count: ctx.cpus,
                gpu_count: ctx.gpus,
                caddy_status,
            };
            match HeartbeatEmitter::spawn(args).await {
                Ok(handle) => {
                    *state.heartbeat_handle.lock().await = Some(handle);
                }
                Err(e) => {
                    error!("Failed setting up heartbeat emitter: {e}");
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            }
        }
        _ => info!("Not emitting heartbeats since the necessary config wasn't provided"),
    };

    ComposeMonitor::spawn(ctx, request.acme, request.docker, request.domain);
    StatusCode::OK
}
