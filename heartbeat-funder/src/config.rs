use alloy::{
    primitives::{Address, U256},
    signers::local::PrivateKeySigner,
};
use serde::{Deserialize, Deserializer, de::Unexpected};
use serde_with::{DisplayFromStr, DurationSeconds, serde_as};
use std::{fmt, time::Duration};

#[serde_as]
#[derive(Deserialize)]
pub struct Config {
    /// A list of static wallets to be funded.
    pub wallets: Vec<Address>,

    /// The funding threshold configurations.
    pub thresholds: ThresholdsConfig,

    /// The RPC config.
    pub rpc: RpcConfig,

    /// The interval at which wallets are polled and funded.
    #[serde(default = "default_interval")]
    #[serde_as(as = "DurationSeconds")]
    pub interval_seconds: Duration,

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

fn default_interval() -> Duration {
    Duration::from_secs(60 * 10)
}
