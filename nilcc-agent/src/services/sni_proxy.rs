use crate::config::SniProxyConfigTimeouts;
use anyhow::{Context as anyhowContext, Result};
use serde::Serialize;
use tera::{Context, Tera};

const HAPROXY_TEMPLATE: &str = include_str!("../templates/haproxy.cfg.j2");

#[derive(Serialize)]
pub struct SniProxyWorkload {
    pub id: String,
    pub domain: String,
    pub http_address: String,
    pub https_address: String,
}

#[derive(Serialize)]
struct SniProxyTemplateContext {
    max_connections: u64,
    timeouts: SniProxyConfigTimeouts,
    workloads: Vec<SniProxyWorkload>,
}

impl SniProxyTemplateContext {
    fn render_config_file(&self) -> Result<String> {
        let context = Context::from_serialize(self)?;
        Tera::one_off(HAPROXY_TEMPLATE, &context, false).context("Error creating config")
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait SniProxyService: Send + Sync {
    /// create or update the SNI proxy configuration based on the provided workloads and reload the proxy service.
    fn update_config(&self, workloads: Vec<SniProxyWorkload>) -> Result<()>;
}

pub struct HaProxySniProxyService {
    config_file_path: String,
    ha_proxy_config_reload_command: String,
    timeouts: SniProxyConfigTimeouts,
    max_connections: u64,
}

impl HaProxySniProxyService {
    pub fn new(
        config_file_path: String,
        ha_proxy_config_reload_command: String,
        timeouts: SniProxyConfigTimeouts,
        max_connections: u64,
    ) -> Self {
        Self { config_file_path, ha_proxy_config_reload_command, timeouts, max_connections }
    }

    pub fn reload(&self) -> Result<()> {
        std::process::Command::new("bash")
            .arg("-c")
            .arg(self.ha_proxy_config_reload_command.clone())
            .status()
            .context("Failed to reload HAProxy configuration")?;
        Ok(())
    }
}

impl SniProxyService for HaProxySniProxyService {
    fn update_config(&self, workloads: Vec<SniProxyWorkload>) -> Result<()> {
        let context = SniProxyTemplateContext {
            max_connections: self.max_connections,
            timeouts: self.timeouts.clone(),
            workloads,
        };
        let config_file = context.render_config_file()?;
        std::fs::write(&self.config_file_path, config_file).context("Failed to write HAProxy config file")?;
        self.reload()
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
    use_backend workload-http-workload1 if { hdr(host) -i workload1.nilcc.com }

# Frontend for HTTPS traffic (port 443)
frontend https_frontend
    bind *:443
    mode tcp
    option tcplog

    # SNI-based routing rules
    tcp-request inspect-delay 5s
    tcp-request content accept if { req_ssl_hello_type 1 }

    # Route based on SNI
    use_backend workload-https-workload1 if { req.ssl_sni -i workload1.nilcc.com }

# Backend servers
backend workload-http-workload1
    mode tcp
    balance roundrobin
    server cvm 127.0.0.1:9000 check
backend workload-https-workload1
    mode tcp
    balance roundrobin
    server cvm 127.0.0.1:9001 check
"#;
    #[test]
    fn test_render_config_file() {
        let config = SniProxyTemplateContext {
            max_connections: 100000,
            timeouts: SniProxyConfigTimeouts { connect: 5000, server: 50000, client: 50000 },
            workloads: vec![SniProxyWorkload {
                id: "workload1".to_string(),
                domain: "workload1.nilcc.com".to_string(),
                http_address: "127.0.0.1:9000".to_string(),
                https_address: "127.0.0.1:9001".to_string(),
            }],
        };
        let config_file = config.render_config_file().unwrap();
        assert_eq!(EXPECTED_HAPROXY_CONFIG, config_file);
    }
}
