use alloy::{
    primitives::{Address, U256},
    signers::local::PrivateKeySigner,
};
use reqwest::Url;
use serde::{Deserialize, Deserializer, de::Unexpected};
use serde_with::{DisplayFromStr, DurationSeconds, serde_as};
use std::{collections::BTreeSet, fmt, time::Duration};

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
}

#[derive(Deserialize)]
pub struct EthThresholdConfig {
    /// The minimum amount of ETH a wallet is tolerated to hold.
    #[serde(deserialize_with = "parse_eth")]
    pub minimum: U256,

    /// The target amount of ETH that we target whenever we fund. When the amount falls bellow
    /// `minimum`, it will be topped up to `target`.
    #[serde(deserialize_with = "parse_eth")]
    pub target: U256,
}

#[derive(Deserialize)]
pub struct RpcConfig {
    /// The RPC endpoint to use.
    pub endpoint: String,
}

fn parse_eth<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;

    impl<'a> serde::de::Visitor<'a> for Visitor {
        type Value = U256;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("struct U256")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            alloy::primitives::utils::parse_ether(v)
                .map_err(|_| serde::de::Error::invalid_value(Unexpected::Str(v), &self))
        }
    }

    deserializer.deserialize_str(Visitor)
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
