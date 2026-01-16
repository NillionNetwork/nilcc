use crate::config::ThresholdsConfig;
use alloy::{
    primitives::{Address, TxKind, utils::format_ether},
    providers::{ProviderBuilder, WsConnect},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use alloy_provider::Provider;
use anyhow::Context;
use std::{collections::BTreeSet, time::Duration};
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender, channel},
    time::{Interval, MissedTickBehavior, interval, sleep},
};
use tracing::{error, info, warn};

const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub struct FunderArgs {
    pub rpc_endpoint: String,
    pub signer: PrivateKeySigner,
    pub static_addresses: BTreeSet<Address>,
    pub poll_interval: Duration,
    pub thresholds: ThresholdsConfig,
}

pub struct Funder {
    rpc_endpoint: String,
    signer: PrivateKeySigner,
    ticker: Interval,
    thresholds: ThresholdsConfig,
    command_receiver: Receiver<FunderCommand>,
    addresses: BTreeSet<Address>,
}

impl Funder {
    pub fn spawn(args: FunderArgs) -> FunderHandle {
        let FunderArgs { rpc_endpoint, signer, static_addresses, poll_interval, thresholds } = args;
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let (sender, command_receiver) = channel(1024);
        let funder = Self { rpc_endpoint, signer, ticker, thresholds, command_receiver, addresses: static_addresses };
        tokio::spawn(funder.run());
        FunderHandle(sender)
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
            select! {
                _ = self.ticker.tick() => {
                    self.handle_tick(&provider).await;
                }
                cmd = self.command_receiver.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd, &provider).await,
                        None => {
                            warn!("Command senders dropped, exiting");
                            break;
                        },
                    }
                }
            };
        }
    }

    async fn handle_tick(&mut self, provider: &impl Provider) {
        info!("Validating that {} addresses are well funded", self.addresses.len());
        if let Err(e) = self.ensure_addresses_funded(&provider).await {
            error!("Failed to fund addresses: {e:#}");
        }
    }

    async fn handle_command(&mut self, command: FunderCommand, provider: &impl Provider) {
        match command {
            FunderCommand::AddAddress(address) => {
                info!("Adding address {address} to monitored set");
                self.addresses.insert(address);
                if let Err(e) = self.ensure_address_funded(address, provider).await {
                    error!("Failed to fund address {address}: {e}");
                }
            }
            FunderCommand::RemoveAddress(address) => {
                info!("Removing address {address} from monitored set");
                self.addresses.remove(&address);
            }
        }
    }

    async fn ensure_addresses_funded(&self, provider: &impl Provider) -> anyhow::Result<()> {
        for address in &self.addresses {
            self.ensure_address_funded(*address, provider).await?;
        }
        Ok(())
    }

    async fn ensure_address_funded(&self, address: Address, provider: &impl Provider) -> anyhow::Result<()> {
        self.ensure_address_funded_eth(address, provider).await?;
        Ok(())
    }

    async fn ensure_address_funded_eth(&self, address: Address, provider: &impl Provider) -> anyhow::Result<()> {
        let eth_balance = provider.get_balance(address).await.context("Failed to get address balance")?;
        let eth_balance_str = format_ether(eth_balance);
        if eth_balance > self.thresholds.eth.minimum {
            info!("Address {address} has enough ETH balance: {eth_balance_str}");
            return Ok(());
        }
        let missing = self.thresholds.eth.target.saturating_sub(eth_balance);
        let missing_str = format_ether(missing);
        info!("{address} has {eth_balance_str} ETH, need to fund with {missing_str} ETH");

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

#[derive(Clone)]
pub struct FunderHandle(Sender<FunderCommand>);

impl FunderHandle {
    pub(crate) async fn add_address(&self, address: Address) {
        self.send(FunderCommand::AddAddress(address)).await;
    }

    pub(crate) async fn remove_address(&self, address: Address) {
        self.send(FunderCommand::RemoveAddress(address)).await;
    }

    async fn send(&self, command: FunderCommand) {
        if self.0.send(command).await.is_err() {
            error!("Funder receiver dropped");
        }
    }
}

enum FunderCommand {
    AddAddress(Address),
    RemoveAddress(Address),
}
