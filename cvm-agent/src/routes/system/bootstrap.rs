use crate::{
    caddy::CaddyMonitor,
    routes::{SharedState, SystemState},
};
use axum::{http::StatusCode, Json};
use cvm_agent_models::bootstrap::{BootstrapRequest, CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use tokio::process::Command;

const COMPOSE_PROJECT_NAME: &str = "cvm";

pub(crate) async fn handler(state: SharedState, request: Json<BootstrapRequest>) -> StatusCode {
    let mut system_state = state.system_state.lock().unwrap();
    if !matches!(&*system_state, SystemState::WaitingBootstrap) {
        return StatusCode::OK;
    }
    let ctx = state.context.clone();
    let mut command = Command::new("docker");
    let command = command
        .current_dir(&ctx.iso_mount)
        // pass in `FILES` which points to `<iso>/files`
        .env("FILES", ctx.external_files.into_os_string())
        // pass in other env vars that are needed by our compose file
        .env("CADDY_INPUT_FILE", ctx.caddy_config.into_os_string())
        .env("NILCC_VERSION", ctx.version)
        .env("NILCC_VM_TYPE", ctx.vm_type)
        .env(CADDY_ACME_EAB_KEY_ID, &request.acme_eab_key_id)
        .env(CADDY_ACME_EAB_MAC_KEY, &request.acme_eab_mac_key)
        .arg("compose")
        // set a well defined project name, this is used as a prefix for container names
        .arg("-p")
        .arg(COMPOSE_PROJECT_NAME)
        // point to the user provided compose file first
        .arg("-f")
        .arg(ctx.user_docker_compose)
        // then ours
        .arg("-f")
        .arg(ctx.system_docker_compose)
        .arg("up");
    match command.spawn() {
        Ok(child) => {
            *system_state = SystemState::Starting(child);
            CaddyMonitor::spawn(state.docker.clone(), state.system_state.clone());
            StatusCode::OK
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
