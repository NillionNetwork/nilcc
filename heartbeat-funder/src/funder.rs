use crate::{
    config::{ContractsConfig, ThresholdsConfig},
    funder::NilToken::NilTokenInstance,
};
use alloy::{
    primitives::{
        Address, ParseSignedError, TxKind, U256,
        utils::{ParseUnits, Unit, UnitsError, format_units, parse_units},
    },
    providers::{ProviderBuilder, WsConnect},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
};
use alloy_provider::{DynProvider, Provider};
use anyhow::{Context as _, bail};
use std::{collections::BTreeSet, fmt, str::FromStr, time::Duration};
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender, channel},
    time::{Interval, MissedTickBehavior, interval, sleep},
};
use tracing::{error, info, warn};

const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(10);

// Generate type-safe contract bindings from ABI
sol!(
    #[sol(rpc)]
    #[derive(Debug)]
    contract NilToken {
        function balanceOf(address account) public view virtual returns (uint256);
        function transfer(address to, uint256 value) public virtual returns (bool);
    }
);

pub struct FunderArgs {
    pub rpc_endpoint: String,
    pub signer: PrivateKeySigner,
    pub static_addresses: BTreeSet<Address>,
    pub poll_interval: Duration,
    pub thresholds: ThresholdsConfig,
    pub contracts: ContractsConfig,
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
        let FunderArgs { rpc_endpoint, signer, static_addresses, poll_interval, thresholds, contracts } = args;
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let (sender, command_receiver) = channel(1024);
        let funder = Self { rpc_endpoint, signer, ticker, thresholds, command_receiver, addresses: static_addresses };
        tokio::spawn(funder.run(contracts));
        FunderHandle(sender)
    }

    async fn run(mut self, contracts: ContractsConfig) {
        info!("Using wallet {}", self.signer.address());

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
        let provider = DynProvider::new(provider);
        let nil = NilTokenInstance::new(contracts.nil, provider.clone());
        let ctx = Context { provider, nil };

        loop {
            select! {
                _ = self.ticker.tick() => {
                    self.handle_tick(&ctx).await;
                }
                cmd = self.command_receiver.recv() => {
                    match cmd {
                        Some(cmd) => self.handle_command(cmd, &ctx).await,
                        None => {
                            warn!("Command senders dropped, exiting");
                            break;
                        },
                    }
                }
            };
        }
    }

    async fn handle_tick(&mut self, ctx: &Context) {
        // Print our balance so we can keep track of this externally.
        let address = self.signer.address();
        match ctx.provider.get_balance(address).await.map(EthAmount::from) {
            Ok(balance) => info!("Wallet {address} has {balance} ETH"),
            Err(e) => {
                error!("Failed to get our own balance: {e}");
            }
        };

        info!("Validating that {} addresses are well funded", self.addresses.len());
        if let Err(e) = self.ensure_addresses_funded(ctx).await {
            error!("Failed to fund addresses: {e:#}");
        }
    }

    async fn handle_command(&mut self, command: FunderCommand, ctx: &Context) {
        match command {
            FunderCommand::AddAddress(address) => {
                info!("Adding address {address} to monitored set");
                self.addresses.insert(address);
                if let Err(e) = self.ensure_address_funded(address, ctx).await {
                    error!("Failed to fund address {address}: {e}");
                }
            }
            FunderCommand::RemoveAddress(address) => {
                info!("Removing address {address} from monitored set");
                self.addresses.remove(&address);
            }
        }
    }

    async fn ensure_addresses_funded(&self, ctx: &Context) -> anyhow::Result<()> {
        for address in &self.addresses {
            self.ensure_address_funded(*address, ctx).await?;
        }
        Ok(())
    }

    async fn ensure_address_funded(&self, address: Address, ctx: &Context) -> anyhow::Result<()> {
        self.ensure_address_funded_eth(address, ctx).await?;
        self.ensure_address_funded_nil(address, ctx).await?;
        Ok(())
    }

    async fn ensure_address_funded_eth(&self, address: Address, ctx: &Context) -> anyhow::Result<()> {
        let eth_balance: EthAmount =
            ctx.provider.get_balance(address).await.context("Failed to get address balance")?.into();
        if eth_balance > self.thresholds.eth.minimum {
            info!("Address {address} has enough ETH balance: {eth_balance}");
            return Ok(());
        }
        let missing = self.thresholds.eth.target.saturating_sub(eth_balance);
        info!("{address} has {eth_balance} ETH, need to fund with {missing} ETH");

        let tx = TransactionRequest { to: Some(TxKind::Call(address)), value: Some(missing.0), ..Default::default() };
        let tx_hash = ctx.provider.send_transaction(tx).await?.watch().await?;
        info!("Funded {address} with {missing} ETH in transaction {tx_hash}");
        Ok(())
    }

    async fn ensure_address_funded_nil(&self, address: Address, ctx: &Context) -> anyhow::Result<()> {
        let nil_balance: NilAmount =
            ctx.nil.balanceOf(address).call().await.context("Failed to get address balance")?.into();
        if nil_balance > self.thresholds.nil.minimum {
            info!("Address {address} has enough NIL balance: {nil_balance}");
            return Ok(());
        }
        let missing = self.thresholds.nil.target.saturating_sub(nil_balance);
        info!("{address} has {nil_balance} NIL, need to fund with {missing} NIL");

        let receipt = ctx
            .nil
            .transfer(address, missing.0)
            .send()
            .await
            .context("Failed to send NIL transfer transaction")?
            .get_receipt()
            .await
            .context("Failed to get TX receipt")?;
        if receipt.status() {
            let tx_hash = receipt.transaction_hash;
            info!("Funded {address} with {missing} NIL in transaction {tx_hash}");
            Ok(())
        } else {
            bail!("NIL transaction submission failed")
        }
    }

    async fn connect(&self) -> anyhow::Result<impl Provider + Clone + use<>> {
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

struct Context {
    provider: DynProvider,
    nil: NilTokenInstance<DynProvider>,
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

pub type EthAmount = AssetAmount<{ Unit::ETHER.get() }>;
pub type NilAmount = AssetAmount<6>;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct AssetAmount<const U: u8>(U256);

impl<const U: u8> AssetAmount<U> {
    fn saturating_sub(self, other: AssetAmount<U>) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl<const U: u8> FromStr for AssetAmount<U> {
    type Err = UnitsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = parse_units(s, U)?;
        match value {
            ParseUnits::U256(value) => Ok(Self(value)),
            ParseUnits::I256(_) => Err(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow)),
        }
    }
}

impl<const U: u8> fmt::Display for AssetAmount<U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted = format_units(self.0, U).expect("formatting units failed");
        write!(f, "{formatted}")
    }
}

impl<const U: u8> From<U256> for AssetAmount<U> {
    fn from(amount: U256) -> Self {
        Self(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ether() {
        let input = "0.000000000000001337";
        let value: EthAmount = input.parse().unwrap();
        let output = format!("{value}");
        assert_eq!(output, input);
    }

    #[test]
    fn nil() {
        let input = "0.001337";
        let value: NilAmount = input.parse().unwrap();
        let output = format!("{value}");
        assert_eq!(output, input);
    }
}
