use crate::verify::Processor;
use anyhow::Context;
use serde::Deserialize;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub attestation: AttestationConfig,
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

#[derive(Deserialize, Default)]
pub struct AttestationConfig {
    #[serde(default)]
    pub processor: Option<Processor>,
}

fn default_bind_endpoint() -> SocketAddr {
    // 0.0.0.0:8080
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8080))
}
