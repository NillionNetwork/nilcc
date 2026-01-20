use crate::funder::{EthAmount, NilAmount};
use alloy::{primitives::Address, signers::local::PrivateKeySigner};
use reqwest::Url;
use serde::Deserialize;
use serde_with::{DisplayFromStr, DurationSeconds, serde_as};
use std::{collections::BTreeSet, time::Duration};

#[serde_as]
#[derive(Deserialize)]
pub struct Config {
    /// A list of static wallets to be funded.
    #[serde(default)]
    pub wallets: BTreeSet<Address>,

    /// Configuration for standalone nilcc-agent instances that need to be monitored.
    #[serde(default)]
    pub agents: Vec<AgentConfig>,

    /// Configuration for nilcc-api.
    #[serde(default)]
    pub api: Option<ApiConfig>,

    /// The funding threshold configurations.
    pub thresholds: ThresholdsConfig,

    /// The RPC config.
    pub rpc: RpcConfig,

    /// The contracts addresses.
    pub contracts: ContractsConfig,

    /// The interval configuration.
    #[serde(default)]
    pub intervals: IntervalsConfig,

    /// The private key to use to sign transactions.
    #[serde_as(as = "DisplayFromStr")]
    pub private_key: PrivateKeySigner,
}

impl Config {
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let mut builder = config::Config::builder().add_source(config::Environment::with_prefix("APP").separator("__"));
        if let Some(path) = path {
            builder = builder.add_source(config::File::new(path, config::FileFormat::Yaml));
        }
        let config = builder.build()?;
        let config = config.try_deserialize()?;
        Ok(config)
    }
}

#[serde_as]
#[derive(Deserialize)]
pub struct IntervalsConfig {
    /// The interval at which wallets are polled and funded.
    #[serde(default = "default_funding_interval")]
    #[serde_as(as = "DurationSeconds")]
    pub funding: Duration,

    /// The interval at which agents are polled for new addresses.
    #[serde(default = "default_agent_interval")]
    #[serde_as(as = "DurationSeconds")]
    pub agent: Duration,

    /// The interval at which nilcc-api is polled for new agents.
    #[serde(default = "default_api_interval")]
    #[serde_as(as = "DurationSeconds")]
    pub api: Duration,
}

impl Default for IntervalsConfig {
    fn default() -> Self {
        Self { funding: default_funding_interval(), agent: default_agent_interval(), api: default_api_interval() }
    }
}

#[derive(Deserialize)]
pub struct AgentConfig {
    /// The agent URL.
    pub url: Url,

    /// The authentication token.
    pub token: String,
}

#[derive(Deserialize)]
pub struct ApiConfig {
    /// The api URL.
    pub url: Url,

    /// The authentication token.
    pub token: String,
}

#[derive(Deserialize)]
pub struct ThresholdsConfig {
    /// The ETH threshold configuration.
    pub eth: EthThresholdConfig,

    /// The NIL threshold configuration.
    pub nil: NilThresholdConfig,
}

#[serde_as]
#[derive(Deserialize)]
pub struct EthThresholdConfig {
    /// The minimum amount of ETH a wallet is tolerated to hold.
    #[serde_as(as = "DisplayFromStr")]
    pub minimum: EthAmount,

    /// The target amount of ETH that we target whenever we fund. When the amount falls bellow
    /// `minimum`, it will be topped up to `target`.
    #[serde_as(as = "DisplayFromStr")]
    pub target: EthAmount,
}

#[serde_as]
#[derive(Deserialize)]
pub struct NilThresholdConfig {
    /// The minimum amount of NIL a wallet is tolerated to hold.
    #[serde_as(as = "DisplayFromStr")]
    pub minimum: NilAmount,

    /// The target amount of NIL that we target whenever we fund. When the amount falls bellow
    /// `minimum`, it will be topped up to `target`.
    #[serde_as(as = "DisplayFromStr")]
    pub target: NilAmount,
}

#[derive(Deserialize)]
pub struct RpcConfig {
    /// The RPC endpoint to use.
    pub endpoint: String,
}

#[derive(Deserialize)]
pub struct ContractsConfig {
    /// The NIL contract address.
    pub nil: Address,
}

fn default_funding_interval() -> Duration {
    Duration::from_secs(60 * 10)
}

fn default_agent_interval() -> Duration {
    Duration::from_secs(30)
}

fn default_api_interval() -> Duration {
    Duration::from_secs(120)
}
