use crate::routes::BootstrapContext;
use anyhow::{bail, Context};
use cvm_agent_models::bootstrap::{AcmeCredentials, DockerCredentials, CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use std::{io, process::Stdio, time::Duration};
use tokio::{
    fs,
    io::AsyncWriteExt,
    process::{Child, Command},
    time::sleep,
};
use tracing::{error, info, warn};

const COMPOSE_PROJECT_NAME: &str = "cvm";
const PULL_RETRY_INTERVAL: Duration = Duration::from_secs(60);
const LAUNCH_RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub(crate) struct ComposeMonitor {
    ctx: BootstrapContext,
    acme: AcmeCredentials,
    docker: Vec<DockerCredentials>,
    domain: String,
}

impl ComposeMonitor {
    pub(crate) fn spawn(ctx: BootstrapContext, acme: AcmeCredentials, docker: Vec<DockerCredentials>, domain: String) {
        let monitor = ComposeMonitor { ctx, acme, docker, domain };
        info!("Spawning docker compose monitor");
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    async fn run(self) {
        loop {
            info!("Pulling docker images");
            match self.pull_images().await {
                Ok(_) => {
                    info!("Images pulled successfully");
                    break;
                }
                Err(e) => {
                    error!("Failed to pull images: {e:#}");
                    self.report_error(e.to_string());
                    info!("Sleeping for {LAUNCH_RETRY_INTERVAL:?}");
                    sleep(PULL_RETRY_INTERVAL).await;
                }
            }
        }
        loop {
            info!("Launching docker compose");
            match self.launch_compose().await {
                Ok(child) => {
                    info!("docker compose is running, waiting for process to exit");
                    if self.check_process_output(child).await {
                        info!("Exiting loop since docker compose is running");
                        break;
                    }
                }
                Err(e) => {
                    self.report_error(e.to_string());
                    error!("Failed to run docker compose: {e}");
                }
            };
            info!("Sleeping for {LAUNCH_RETRY_INTERVAL:?}");
            sleep(LAUNCH_RETRY_INTERVAL).await;
        }
    }

    async fn docker_login(&self, credentials: &DockerCredentials) -> anyhow::Result<()> {
        let registry = match &credentials.server {
            Some(server) => server.as_str(),
            None => "docker hub",
        };
        info!("Logging in to {registry}");

        let mut command = Command::new("docker");
        let mut command = command
            .arg("--config")
            .arg(&self.ctx.docker_config)
            .arg("login")
            .arg("-u")
            .arg(&credentials.username)
            .arg("--password-stdin")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(server) = &credentials.server {
            command = command.arg(server);
        }
        let mut child = command.spawn().context("Failed to invoke docker login")?;
        {
            let mut stdin = child.stdin.take().expect("no stdin");
            stdin.write_all(credentials.password.as_bytes()).await.context("Failed to write docker login password")?;
        }
        let output = child.wait_with_output().await.context("Failed to wait for docker login")?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = Self::extract_stderr_message(&stderr);
            bail!("docker login failed: {message}")
        }
    }

    async fn pull_images(&self) -> anyhow::Result<()> {
        for credential in &self.docker {
            self.docker_login(credential).await.context("Failed to docker login")?;
        }
        info!("Running docker compose pull");
        let output = self
            .base_docker_command()
            .arg("pull")
            .arg("-q")
            .output()
            .await
            .context("Failed to run docker compose pull")?;
        if output.status.success() {
            if !self.docker.is_empty() {
                info!("Removing docker config.json file");
                // this isn't needed anymore because we already pulled images
                fs::remove_file(&self.ctx.docker_config.join("config.json"))
                    .await
                    .context("Failed to remove docker config")?;
            }
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = Self::extract_stderr_message(&stderr);
            bail!("docker compose pull failed: {message}")
        }
    }

    fn extract_stderr_message(stderr: &str) -> &str {
        for line in stderr.lines() {
            // Try to grab the nicer error if we can
            if let Some(error) = line.strip_prefix("Error response from daemon: ") {
                return error;
            }
        }
        stderr
    }

    async fn launch_compose(&self) -> io::Result<Child> {
        self.base_docker_command().arg("up").arg("-d").arg("--no-build").spawn()
    }

    fn base_docker_command(&self) -> Command {
        let mut command = Command::new("docker");
        command
            .current_dir(&self.ctx.iso_mount)
            // pass in `FILES` which points to `<iso>/files`
            .env("FILES", self.ctx.external_files.as_os_str())
            // pass in other env vars that are needed by our compose file
            .env("CADDY_INPUT_FILE", self.ctx.caddy_config.as_os_str())
            .env("NILCC_VERSION", &self.ctx.version)
            .env("NILCC_VM_TYPE", self.ctx.vm_type.to_string())
            .env("NILCC_DOMAIN", &self.domain)
            .env(CADDY_ACME_EAB_KEY_ID, &self.acme.eab_key_id)
            .env(CADDY_ACME_EAB_MAC_KEY, &self.acme.eab_mac_key)
            .stderr(Stdio::piped())
            .arg("--config")
            .arg(&self.ctx.docker_config)
            .arg("compose")
            // set a well defined project name, this is used as a prefix for container names
            .arg("-p")
            .arg(COMPOSE_PROJECT_NAME)
            // point to the user provided compose file first
            .arg("-f")
            .arg(&self.ctx.user_docker_compose)
            // then ours
            .arg("-f")
            .arg(&self.ctx.system_docker_compose);
        command
    }

    async fn check_process_output(&self, child: Child) -> bool {
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
            let message = Self::extract_stderr_message(&stderr);
            let error = format!("docker compose execution failed: {message}");
            self.report_error(error);
            false
        }
    }

    fn report_error(&self, message: String) {
        error!("{message}");
        self.ctx.error_holder.set(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_error_response() {
        let stderr = r"[+] Pulling 2/2
 ✘ other Error context canceled                                                                  1.2s
 ✘ web Error   manifest for foo:bar not found: manifest unknown: manifest unknown                1.2s
Error response from daemon: manifest for foo:bar not found: manifest unknown: manifest unknown";
        let error = ComposeMonitor::extract_stderr_message(&stderr);
        assert_eq!(error, "manifest for foo:bar not found: manifest unknown: manifest unknown");
    }

    #[test]
    fn default_error_message() {
        let stderr = r"foo
bar";
        let error = ComposeMonitor::extract_stderr_message(&stderr);
        assert_eq!(error, stderr);
    }
}
