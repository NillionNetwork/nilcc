use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
};

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    pub nilcc_version: String,
    pub vm_type: VmType,
    #[serde(default = "default_gpu_attester_path")]
    pub gpu_attester_path: PathBuf,
    #[serde(default = "default_proxy_endpoint")]
    pub proxy_endpoint: String,
    pub attestation_domain: String,
}

impl Config {
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let mut builder = config::Config::builder().add_source(config::Environment::with_prefix("APP").separator("__"));
        if let Some(path) = path {
            builder = builder.add_source(config::File::with_name(path))
        }
        let settings = builder.build().context("parsing config")?;
        settings.try_deserialize().context("deserializing config")
    }
}

#[derive(Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_endpoint")]
    pub bind_endpoint: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { bind_endpoint: default_bind_endpoint() }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmType {
    Cpu,
    Gpu,
}

fn default_bind_endpoint() -> SocketAddr {
    // 0.0.0.0:8080
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8080))
}

fn default_gpu_attester_path() -> PathBuf {
    "/opt/nillion/gpu-attester/main.py".into()
}

fn default_proxy_endpoint() -> String {
    "cvm-nilcc-proxy-1:443".to_string()
}
