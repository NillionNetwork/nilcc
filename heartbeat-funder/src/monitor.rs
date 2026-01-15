use crate::config::ThresholdsConfig;
use alloy::{
    primitives::{Address, TxKind, utils::format_ether},
    providers::{ProviderBuilder, WsConnect},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use alloy_provider::Provider;
use anyhow::Context;
use std::time::Duration;
use tokio::time::{Interval, MissedTickBehavior, interval, sleep};
use tracing::{error, info};

const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub struct MonitorArgs {
    pub rpc_endpoint: String,
    pub signer: PrivateKeySigner,
    pub static_addresses: Vec<Address>,
    pub poll_interval: Duration,
    pub thresholds: ThresholdsConfig,
}

pub struct Monitor {
    rpc_endpoint: String,
    signer: PrivateKeySigner,
    ticker: Interval,
    thresholds: ThresholdsConfig,
    addresses: Vec<Address>,
}

impl Monitor {
    pub fn spawn(args: MonitorArgs) {
        let MonitorArgs { rpc_endpoint, signer, static_addresses, poll_interval, thresholds } = args;
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let monitor = Self { rpc_endpoint, signer, ticker, thresholds, addresses: static_addresses };
        tokio::spawn(monitor.run());
    }

    async fn run(mut self) {
        info!("Connecting to RPC endpoint {}", self.rpc_endpoint);
        let provider = loop {
            match self.connect().await {
                Ok(out) => break out,
                Err(e) => {
                    error!("Failed to connect to provider: {e}");
                    sleep(CONNECT_RETRY_INTERVAL).await;
                }
            }
        };

        loop {
            self.ticker.tick().await;

            info!("Validating that {} addresses are well funded", self.addresses.len());
            if let Err(e) = self.ensure_addresses_funded(&provider).await {
                error!("Failed to fund addresses: {e:#}");
            }
        }
    }

    async fn ensure_addresses_funded(&self, provider: &impl Provider) -> anyhow::Result<()> {
        for address in &self.addresses {
            self.ensure_address_funded_eth(*address, provider).await?;
        }
        Ok(())
    }

    async fn ensure_address_funded_eth(&self, address: Address, provider: &impl Provider) -> anyhow::Result<()> {
        let eth_balance = provider.get_balance(address).await.context("Failed to get address balance")?;
        if eth_balance > self.thresholds.eth.minimum {
            info!("Address {address} has enough ETH balance: {}", format_ether(eth_balance));
            return Ok(());
        }
        let missing = self.thresholds.eth.target.saturating_sub(eth_balance);
        let missing_str = format_ether(missing);
        info!("Need to fund address {address} with {missing_str} ETH");

        let tx = TransactionRequest { to: Some(TxKind::Call(address)), value: Some(missing), ..Default::default() };
        let tx_hash = provider.send_transaction(tx).await?.watch().await?;
        info!("Funded {address} with {missing_str} ETH in transaction {tx_hash}");
        Ok(())
    }

    async fn connect(&self) -> anyhow::Result<impl Provider + use<>> {
        let ws = WsConnect::new(&self.rpc_endpoint).with_max_retries(u32::MAX);
        let provider = ProviderBuilder::new()
            .wallet(self.signer.clone())
            .with_simple_nonce_management()
            .with_gas_estimation()
            .connect_ws(ws)
            .await?;
        info!("Connected to RPC endpoint {}", self.rpc_endpoint);
        Ok(provider)
    }
}
