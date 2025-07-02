use crate::config::SniProxyConfigTimeouts;
use crate::repositories::workload::WorkloadModel;
use anyhow::{bail, Context as anyhowContext, Result};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::BTreeMap;
use tera::{Context, Tera};
use tokio::{process::Command, sync::Mutex};
use tracing::error;
use uuid::Uuid;

const HAPROXY_TEMPLATE: &str = include_str!("../templates/haproxy.cfg.j2");

#[derive(Debug, PartialEq)]
pub struct ProxiedVm {
    pub(crate) id: Uuid,
    pub(crate) http_port: u16,
    pub(crate) https_port: u16,
}

impl From<&WorkloadModel> for ProxiedVm {
    fn from(workload: &WorkloadModel) -> Self {
        Self { id: workload.id, http_port: workload.metal_http_port, https_port: workload.metal_https_port }
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ProxyService: Send + Sync {
    /// Add a new proxied VM.
    async fn add_proxied_vm(&self, vm: ProxiedVm);
}

pub struct HaProxyProxyService {
    config_file_path: String,
    ha_proxy_config_reload_command: String,
    timeouts: SniProxyConfigTimeouts,
    dns_subdomain: String,
    max_connections: u64,
    proxied_vms: Mutex<BTreeMap<Uuid, ProxiedVm>>,
}

impl HaProxyProxyService {
    pub fn new(
        config_file_path: String,
        ha_proxy_config_reload_command: String,
        timeouts: SniProxyConfigTimeouts,
        dns_subdomain: String,
        max_connections: u64,
        proxied_vms: Vec<ProxiedVm>,
    ) -> Self {
        let proxied_vms: BTreeMap<_, _> = proxied_vms.into_iter().map(|vm| (vm.id, vm)).collect();
        Self {
            config_file_path,
            ha_proxy_config_reload_command,
            timeouts,
            dns_subdomain,
            max_connections,
            proxied_vms: proxied_vms.into(),
        }
    }

    async fn reload(&self) -> Result<()> {
        Command::new("bash")
            .arg("-c")
            .arg(self.ha_proxy_config_reload_command.clone())
            .status()
            .await
            .context("Failed to reload HAProxy configuration")?;
        Ok(())
    }

    async fn check_config(&self) -> Result<()> {
        let output = Command::new("haproxy").arg("-c").arg("-f").arg(&self.config_file_path).output().await?;
        if !output.status.success() {
            let stderr_message = String::from_utf8_lossy(&output.stderr);
            let stdout_message = String::from_utf8_lossy(&output.stdout);
            bail!("HAProxy configuration check failed: \nSTDOUT: \n{stdout_message}\nSTDERR:\n{stderr_message}");
        }
        Ok(())
    }

    async fn persist_config(&self, proxied_vms: impl IntoIterator<Item = &ProxiedVm>) -> Result<()> {
        let dns_subdomain = &self.dns_subdomain;
        let workloads: Vec<_> = proxied_vms
            .into_iter()
            .map(|vm| {
                let ProxiedVm { id, http_port, https_port } = vm;
                ProxyBackend {
                    id: id.to_string(),
                    domain: format!("{id}.{dns_subdomain}"),
                    http_address: format!("127.0.0.1:{http_port}"),
                    https_address: format!("127.0.0.1:{https_port}"),
                }
            })
            .collect();
        let context = SniProxyTemplateContext {
            max_connections: self.max_connections,
            timeouts: self.timeouts.clone(),
            backends: workloads,
        };
        let config_file = context.render_config_file()?;
        tokio::fs::write(&self.config_file_path, config_file).await.context("Failed to write HAProxy config file")?;
        self.check_config().await?;
        self.reload().await
    }
}

#[async_trait]
impl ProxyService for HaProxyProxyService {
    async fn add_proxied_vm(&self, vm: ProxiedVm) {
        let mut proxied_vms = self.proxied_vms.lock().await;
        proxied_vms.insert(vm.id, vm);
        if let Err(e) = self.persist_config(proxied_vms.values()).await {
            error!("Failed to persist configuration: {e}");
        }
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
    const EXPECTED_HAPROXY_CONFIG: &str = r#"global
    daemon
    maxconn 100000
    log stdout local0 info

defaults
    mode tcp
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 5000050000ms
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

    # Route based on SNI
    use_backend backend-https-foo if { req.ssl_sni -i foo.nilcc.com }

# Backend servers
backend backend-http-foo
    mode tcp
    balance roundrobin
    server cvm 127.0.0.1:9000 check
backend backend-https-foo
    mode tcp
    balance roundrobin
    server cvm 127.0.0.1:9001 check

"#;
    #[test]
    fn render_config_file() {
        let config = SniProxyTemplateContext {
            max_connections: 100000,
            timeouts: SniProxyConfigTimeouts { connect: 5000, server: 50000, client: 50000 },
            backends: vec![ProxyBackend {
                id: "foo".to_string(),
                domain: "foo.nilcc.com".to_string(),
                http_address: "127.0.0.1:9000".to_string(),
                https_address: "127.0.0.1:9001".to_string(),
            }],
        };
        let config_file = config.render_config_file().unwrap();
        assert_eq!(EXPECTED_HAPROXY_CONFIG, config_file);
    }
}
