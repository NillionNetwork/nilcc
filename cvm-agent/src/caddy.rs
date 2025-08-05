use crate::routes::SystemState;
use bollard::{
    container::LogOutput,
    query_parameters::{LogsOptionsBuilder, RestartContainerOptionsBuilder},
    Docker,
};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use std::{
    borrow::Cow,
    mem,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

const CONTAINER_NAME: &str = "cvm-nilcc-proxy-1";
const LOOP_INTERVAL: Duration = Duration::from_secs(10);

/// A monitor for the caddy container.
pub struct CaddyMonitor {
    docker: Docker,
    system_state: Arc<Mutex<SystemState>>,
}

impl CaddyMonitor {
    pub fn spawn(docker: Docker, system_state: Arc<Mutex<SystemState>>) {
        let monitor = Self { docker, system_state };
        info!("Spawning caddy monitor");
        tokio::spawn(async move {
            monitor.run().await;
        });
    }

    async fn run(self) {
        let mut threshold_timestamp = 0.0;
        loop {
            let builder = LogsOptionsBuilder::new().tail("10").stderr(true);
            let stream = self.docker.logs(CONTAINER_NAME, Some(builder.build()));
            let (next_timestamp, status) = Self::check_caddy_status(stream, threshold_timestamp).await;
            threshold_timestamp = next_timestamp;
            match status {
                Status::Ok => {
                    let mut system_state = self.system_state.lock().unwrap();
                    match mem::take(&mut *system_state) {
                        SystemState::WaitingBootstrap => error!("System is still waiting for bootstrap"),
                        SystemState::Starting(child) | SystemState::Ready(child) => {
                            info!("Caddy is running successfully");
                            *system_state = SystemState::Ready(child)
                        }
                    }
                }
                Status::Unknown => {
                    debug!("Caddy is in an unknown state")
                }
                Status::NeedsRestart => {
                    warn!("Caddy needs to be restarted");
                    let options = RestartContainerOptionsBuilder::new().build();
                    if let Err(e) = self.docker.restart_container(CONTAINER_NAME, Some(options)).await {
                        error!("Failed to restart container: {e}");
                    }
                }
            };
            sleep(LOOP_INTERVAL).await;
        }
    }

    async fn check_caddy_status<T>(mut stream: T, timestamp_threshold: f64) -> (f64, Status)
    where
        T: Stream<Item = Result<LogOutput, bollard::errors::Error>> + Unpin,
    {
        let mut status = Status::Unknown;
        let mut last_timestamp = timestamp_threshold;
        while let Some(output) = stream.next().await {
            let Ok(output) = output else {
                return (timestamp_threshold, Status::Unknown);
            };
            let output = output.into_bytes();
            let line = String::from_utf8_lossy(&output);
            let Ok(line) = serde_json::from_str::<LogLine>(&line) else {
                continue;
            };
            if line.ts <= timestamp_threshold {
                continue;
            }
            if line.msg == "certificate obtained successfully" {
                status = Status::Ok;
            } else if line.error.contains("https://acme-staging-v02.api.letsencrypt.org") {
                status = Status::NeedsRestart;
            }
            last_timestamp = line.ts;
        }
        (last_timestamp, status)
    }
}

#[derive(Deserialize)]
struct LogLine<'a> {
    ts: f64,
    msg: Cow<'a, str>,
    #[serde(default)]
    error: Cow<'a, str>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum Status {
    Ok,
    Unknown,
    NeedsRestart,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stream(lines: &[&str]) -> impl Stream<Item = Result<LogOutput, bollard::errors::Error>> {
        let lines = lines.iter().map(|l| Ok(LogOutput::StdErr { message: l.to_string().into() }));
        futures::stream::iter(lines)
    }

    #[tokio::test]
    async fn success() {
        let lines = make_stream(&[
            r#"{"level":"info","ts":1754341166.3218322,"msg":"got renewal info","names":["c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network"],"window_start":1759445101,"window_end":1759600550,"selected_time":1759500028,"recheck_after":1754362766.3218107,"explanation_url":""}"#,
            r#"{"level":"info","ts":1754341166.41461,"msg":"got renewal info","names":["c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network"],"window_start":1759445101,"window_end":1759600550,"selected_time":1759522387,"recheck_after":1754362766.4145963,"explanation_url":""}"#,
            r#"{"level":"info","ts":1754341166.4151092,"msg":"successfully downloaded available certificate chains","count":2,"first_url":"https://acme-v02.api.letsencrypt.org/acme/cert/052316941319dbec1e80b8736a6f7f470c14"}"#,
            r#"{"level":"info","ts":1754341166.425855,"logger":"tls.obtain","msg":"certificate obtained successfully","identifier":"c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network","issuer":"acme-v02.api.letsencrypt.org-directory"}"#,
            r#"{"level":"info","ts":1754341166.4263053,"logger":"tls.obtain","msg":"releasing lock","identifier":"c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network"}"#,
        ]);
        let (timestamp, status) = CaddyMonitor::check_caddy_status(lines, 0.0).await;
        assert_eq!(timestamp, 1754341166.4263053);
        assert_eq!(status, Status::Ok);
    }

    #[tokio::test]
    async fn failure() {
        let lines = make_stream(&[
            r#"{"level":"info","ts":1754340523.052331,"logger":"tls.issuance.acme","msg":"using configured ACME account"}"#,
            r#"{"level":"info","ts":1754340523.1608796,"logger":"tls.issuance.acme","msg":"using ACME account","account_id":"https://acme-v02.api.letsencrypt.org/acme/acct/2563259061","account_contact":[]}"#,
            r#"{"level":"error","ts":1754340523.3471634,"logger":"tls.obtain","msg":"could not get certificate from issuer","identifier":"c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network","issuer":"acme-v02.api.letsencrypt.org-directory","error":"HTTP 400 urn:ietf:params:acme:error:malformed - Unable to validate JWS :: KeyID header contained an invalid account URL: \"https://acme-v02.api.letsencrypt.org/acme/acct/2563259061\""}"#,
            r#"{"level":"error","ts":1754340523.3485954,"logger":"tls.obtain","msg":"will retry","error":"[c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network] Obtain: [c7cd1d31-b890-4438-92da-df931151c4bd.workloads.nilcc.sandbox.nillion.network] creating new order: attempt 1: https://acme-staging-v02.api.letsencrypt.org/acme/new-order: HTTP 400 urn:ietf:params:acme:error:malformed - Unable to validate JWS :: KeyID header contained an invalid account URL: \"https://acme-v02.api.letsencrypt.org/acme/acct/2563259061\" (ca=https://acme-staging-v02.api.letsencrypt.org/directory)","attempt":8,"retrying_in":1200,"elapsed":2402.705797515,"max_duration":2592000}"#,
        ]);
        let (timestamp, status) = CaddyMonitor::check_caddy_status(lines, 0.0).await;
        assert_eq!(timestamp, 1754340523.3485954);
        assert_eq!(status, Status::NeedsRestart);
    }

    #[tokio::test]
    async fn ignore_old_timestamp() {
        let lines = make_stream(&[
            r#"{"level":"info","ts":1754341166.3218322,"msg":"hi"}"#,
            r#"{"level":"info","ts":1754341166.41461,"msg":"bad","error":"https://acme-staging-v02.api.letsencrypt.org"}"#,
            r#"{"level":"info","ts":1754341166.4151092,"msg":"bar"}"#,
        ]);
        let (timestamp, status) = CaddyMonitor::check_caddy_status(lines, 1754341166.415).await;
        // the last timestamp
        assert_eq!(timestamp, 1754341166.4151092);
        // but we don't know the state since nothing is conclusive based on just the last line
        assert_eq!(status, Status::Unknown);
    }
}
