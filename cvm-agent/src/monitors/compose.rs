use crate::routes::BootstrapContext;
use cvm_agent_models::bootstrap::{CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use std::{io, process::Stdio, time::Duration};
use tokio::{
    process::{Child, Command},
    time::sleep,
};
use tracing::{error, info, warn};

const COMPOSE_PROJECT_NAME: &str = "cvm";
const RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub(crate) struct ComposeMonitor {
    ctx: BootstrapContext,
    acme_eab_key_id: String,
    acme_eab_mac_key: String,
}

impl ComposeMonitor {
    pub(crate) fn spawn(ctx: BootstrapContext, acme_eab_key_id: String, acme_eab_mac_key: String) {
        let monitor = ComposeMonitor { ctx, acme_eab_key_id, acme_eab_mac_key };
        info!("Spawning docker compose monitor");
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    async fn run(self) {
        loop {
            info!("Launching docker compose");
            match self.launch_compose().await {
                Ok(child) => {
                    info!("docker compose is running, waiting for process to exit");
                    if Self::check_process_output(child).await {
                        info!("Exiting loop since docker compose is running");
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to run docker compose: {e}");
                }
            };
            info!("Sleeping for {RETRY_INTERVAL:?}");
            sleep(RETRY_INTERVAL).await;
        }
    }

    async fn launch_compose(&self) -> io::Result<Child> {
        let mut command = Command::new("docker");
        let command = command
            .current_dir(&self.ctx.iso_mount)
            // pass in `FILES` which points to `<iso>/files`
            .env("FILES", self.ctx.external_files.as_os_str())
            // pass in other env vars that are needed by our compose file
            .env("CADDY_INPUT_FILE", self.ctx.caddy_config.as_os_str())
            .env("NILCC_VERSION", &self.ctx.version)
            .env("NILCC_VM_TYPE", self.ctx.vm_type.to_string())
            .env(CADDY_ACME_EAB_KEY_ID, &self.acme_eab_key_id)
            .env(CADDY_ACME_EAB_MAC_KEY, &self.acme_eab_mac_key)
            .stderr(Stdio::piped())
            .arg("compose")
            // set a well defined project name, this is used as a prefix for container names
            .arg("-p")
            .arg(COMPOSE_PROJECT_NAME)
            // point to the user provided compose file first
            .arg("-f")
            .arg(&self.ctx.user_docker_compose)
            // then ours
            .arg("-f")
            .arg(&self.ctx.system_docker_compose)
            .arg("up")
            .arg("-d");
        command.spawn()
    }

    async fn check_process_output(child: Child) -> bool {
        let output = match child.wait_with_output().await {
            Ok(output) => output,
            Err(e) => {
                error!("Could not wait for docker compose: {e:?}");
                return false;
            }
        };

        if output.status.success() {
            warn!("docker compose exited successfully");
            true
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("docker compose execution failed: {stderr}");
            false
        }
    }
}
