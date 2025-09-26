use crate::config::SniProxyConfigTimeouts;
use crate::repositories::workload::Workload;
use anyhow::{Context as anyhowContext, Result, bail};
use async_trait::async_trait;
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};
use tera::{Context, Tera};
use tokio::{io::AsyncWriteExt, net::UnixSocket, process::Command, sync::Mutex};
use tracing::{error, info};
use uuid::Uuid;

const HAPROXY_TEMPLATE: &str = include_str!("../../resources/haproxy.cfg.j2");

#[derive(Debug, PartialEq)]
pub struct ProxiedVm {
    pub(crate) id: Uuid,
    pub(crate) domain: String,
    pub(crate) http_port: u16,
    pub(crate) https_port: u16,
}

impl From<&Workload> for ProxiedVm {
    fn from(workload: &Workload) -> Self {
        Self {
            id: workload.id,
            domain: workload.domain.clone(),
            http_port: workload.http_port(),
            https_port: workload.https_port(),
        }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ProxyService: Send + Sync {
    /// Persist the config with whatever state we currently have.
    async fn persist_current_config(&self) -> Result<()>;

    /// Start proxying a VM.
    async fn start_vm_proxy(&self, vm: ProxiedVm);

    /// Stop proxying a VM.
    async fn stop_vm_proxy(&self, id: Uuid);
}

pub struct ProxyServiceArgs {
    pub config_file_path: PathBuf,
    pub master_socket_path: PathBuf,
    pub timeouts: SniProxyConfigTimeouts,
    pub agent_domain: String,
    pub agent_port: u16,
    pub max_connections: u64,
    pub proxied_vms: Vec<ProxiedVm>,
    pub reload_config: bool,
}

pub struct HaProxyProxyService {
    config_file_path: PathBuf,
    master_socket_path: PathBuf,
    timeouts: SniProxyConfigTimeouts,
    agent_domain: String,
    agent_port: u16,
    max_connections: u64,
    reload_config: bool,
    proxied_vms: Mutex<BTreeMap<Uuid, ProxiedVm>>,
}

impl HaProxyProxyService {
    pub fn new(args: ProxyServiceArgs) -> Self {
        let ProxyServiceArgs {
            config_file_path,
            master_socket_path,
            timeouts,
            agent_domain,
            agent_port,
            max_connections,
            proxied_vms,
            reload_config,
        } = args;
        let proxied_vms: BTreeMap<_, _> = proxied_vms.into_iter().map(|vm| (vm.id, vm)).collect();
        Self {
            config_file_path,
            master_socket_path,
            timeouts,
            agent_domain,
            agent_port,
            max_connections,
            reload_config,
            proxied_vms: proxied_vms.into(),
        }
    }

    async fn reload(&self) -> Result<()> {
        let mut socket =
            UnixSocket::new_stream()?.connect(&self.master_socket_path).await.context("Connecting to master socket")?;
        socket.write_all(b"reload\n").await?;
        Ok(())
    }

    async fn validate_config(&self) -> Result<()> {
        let output = Command::new("haproxy").arg("-c").arg("-f").arg(&self.config_file_path).output().await?;
        if !output.status.success() {
            let stderr_message = String::from_utf8_lossy(&output.stderr);
            let stdout_message = String::from_utf8_lossy(&output.stdout);
            bail!("HAProxy configuration check failed: \nSTDOUT: \n{stdout_message}\nSTDERR:\n{stderr_message}");
        }
        Ok(())
    }

    async fn persist_config(&self, proxied_vms: impl IntoIterator<Item = &ProxiedVm>) -> Result<()> {
        let backends: Vec<_> = proxied_vms
            .into_iter()
            .map(|vm| {
                let ProxiedVm { id, domain, http_port, https_port } = vm;
                ProxyBackend {
                    id: id.to_string(),
                    domain: domain.clone(),
                    http_address: format!("127.0.0.1:{http_port}"),
                    https_address: format!("127.0.0.1:{https_port}"),
                }
            })
            .collect();
        let context = SniProxyTemplateContext {
            max_connections: self.max_connections,
            timeouts: self.timeouts.clone(),
            agent_domain: self.agent_domain.clone(),
            agent_port: self.agent_port,
            backends,
        };
        info!("Persisting HA proxy config using {} VMs as backends", context.backends.len());
        let config_file = context.render_config_file()?;
        tokio::fs::write(&self.config_file_path, config_file).await.context("Failed to write HAProxy config file")?;
        if self.reload_config {
            self.validate_config().await.context("Failed to check config")?;
            self.reload().await?;
            info!("HA proxy config reloaded");
        }
        Ok(())
    }
}

#[async_trait]
impl ProxyService for HaProxyProxyService {
    async fn persist_current_config(&self) -> Result<()> {
        let proxied_vms = self.proxied_vms.lock().await;
        self.persist_config(proxied_vms.values()).await
    }

    async fn start_vm_proxy(&self, vm: ProxiedVm) {
        let mut proxied_vms = self.proxied_vms.lock().await;
        proxied_vms.insert(vm.id, vm);
        if let Err(e) = self.persist_config(proxied_vms.values()).await {
            error!("Failed to persist configuration: {e}");
        }
    }

    async fn stop_vm_proxy(&self, id: Uuid) {
        let mut proxied_vms = self.proxied_vms.lock().await;
        proxied_vms.remove(&id);
    }
}

#[derive(Serialize)]
struct ProxyBackend {
    id: String,
    domain: String,
    http_address: String,
    https_address: String,
}

#[derive(Serialize)]
struct SniProxyTemplateContext {
    max_connections: u64,
    timeouts: SniProxyConfigTimeouts,
    backends: Vec<ProxyBackend>,
    agent_domain: String,
    agent_port: u16,
}

impl SniProxyTemplateContext {
    fn render_config_file(&self) -> Result<String> {
        let context = Context::from_serialize(self)?;
        Tera::one_off(HAPROXY_TEMPLATE, &context, false).context("Error creating config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_config_file() {
        let expected_config = r#"global
    daemon
    maxconn 100000
    log stdout local0 info

defaults
    mode tcp
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms
    option tcplog
    log global

# Frontend for HTTP traffic (port 80)
frontend http_frontend
    bind *:80
    mode http
    option httplog

    # Route based on HTTP Host header
    use_backend backend-http-foo if { hdr(host) -i foo.nilcc.com }

# Frontend for HTTPS traffic (port 443)
frontend https_frontend
    bind *:443
    mode tcp
    option tcplog

    # SNI-based routing rules
    tcp-request inspect-delay 5s
    tcp-request content accept if { req_ssl_hello_type 1 }

    # Route via SNI to nilcc-agent
    use_backend agent-backend if { req.ssl_sni -i agent1.example.com }

    # Route based on SNI
    use_backend backend-https-foo if { req.ssl_sni -i foo.nilcc.com }

# Backend servers

# nilcc-agent backend
backend agent-backend 
    mode tcp
    server nilcc-agent 127.0.0.1:8080 check

# VM foo backend servers
backend backend-http-foo
    mode http
    balance roundrobin
    server cvm 127.0.0.1:9000 check

backend backend-https-foo
    mode tcp
    balance roundrobin
    server cvm 127.0.0.1:9001 check

"#;
        let config = SniProxyTemplateContext {
            max_connections: 100000,
            timeouts: SniProxyConfigTimeouts { connect: 5000, server: 50000, client: 50000 },
            agent_domain: "agent1.example.com".into(),
            agent_port: 8080,
            backends: vec![ProxyBackend {
                id: "foo".into(),
                domain: "foo.nilcc.com".into(),
                http_address: "127.0.0.1:9000".into(),
                https_address: "127.0.0.1:9001".into(),
            }],
        };
        let config_file = config.render_config_file().unwrap();
        assert_eq!(config_file, expected_config);
    }
}
