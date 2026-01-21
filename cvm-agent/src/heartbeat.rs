use crate::{
    heartbeat::{HeartbeatManager::HeartbeatManagerInstance, NilToken::NilTokenInstance},
    monitors::caddy::CaddyStatus,
};
use alloy::{
    primitives::{Address, Uint},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol_types::sol,
};
use alloy_provider::{Provider, WsConnect};
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use std::time::Duration;
use tokio::{
    sync::mpsc::{Receiver, Sender, channel},
    time::{Interval, MissedTickBehavior, interval, sleep},
};
use tracing::{error, info, warn};
use uuid::Uuid;

sol! {
    #[sol(rpc)]
    contract HeartbeatManager {
        function submitHeartbeat(bytes calldata rawHTX, uint64 snapshotId) external whenNotPaused nonReentrant returns (bytes32 heartbeatKey);
    }

    #[sol(rpc)]
    contract NilToken {
        function approve(address spender, uint256 value) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
    }
}

const ATTESTATION_PATH: &str = "/nilcc/api/v2/report";
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(10);
const TOKEN_APPROVAL_RETRY_INTERVAL: Duration = Duration::from_secs(30);
const NIL_TOKEN_DECIMALS: u8 = 6;

pub(crate) struct HeartbeatEmitterArgs {
    pub(crate) workload_id: Uuid,
    pub(crate) workload_domain: String,
    pub(crate) rpc_endpoint: String,
    pub(crate) heartbeat_contract_address: String,
    pub(crate) token_contract_address: String,
    pub(crate) wallet_private_key: Vec<u8>,
    pub(crate) nilcc_version: String,
    pub(crate) docker_compose_hash: [u8; 32],
    pub(crate) tick_interval: Duration,
    pub(crate) measurement_hash_url: String,
    pub(crate) cpu_count: u64,
    pub(crate) gpu_count: u64,
    pub(crate) caddy_status: CaddyStatus,
}

pub(crate) struct HeartbeatEmitter {
    workload_id: Uuid,
    attestation_url: String,
    rpc_endpoint: String,
    wallet: PrivateKeySigner,
    heartbeat_contract_address: Address,
    token_contract_address: Address,
    nilcc_version: String,
    docker_compose_hash: [u8; 32],
    tick_interval: Duration,
    measurement_hash_url: String,
    cpu_count: u64,
    gpu_count: u64,
}

impl HeartbeatEmitter {
    pub(crate) async fn spawn(args: HeartbeatEmitterArgs) -> anyhow::Result<HeartbeatEmitterHandle> {
        let HeartbeatEmitterArgs {
            workload_id,
            workload_domain,
            rpc_endpoint,
            heartbeat_contract_address,
            token_contract_address,
            wallet_private_key,
            nilcc_version,
            docker_compose_hash,
            tick_interval,
            measurement_hash_url,
            cpu_count,
            gpu_count,
            caddy_status,
        } = args;
        let (sender, receiver) = channel(1024);
        let heartbeat_contract_address =
            heartbeat_contract_address.parse().context("Invalid heartbeat contract address")?;
        let token_contract_address = token_contract_address.parse().context("Invalid token contract address")?;
        let attestation_url = format!("https://{workload_domain}{ATTESTATION_PATH}");
        let wallet = PrivateKeySigner::from_slice(&wallet_private_key).context("Invalid wallet private key")?;
        info!("Starting heartbeat emitter using wallet {}", wallet.address());

        let submitter = Self {
            workload_id,
            attestation_url,
            rpc_endpoint,
            wallet,
            heartbeat_contract_address,
            token_contract_address,
            nilcc_version,
            docker_compose_hash,
            tick_interval,
            measurement_hash_url,
            cpu_count,
            gpu_count,
        };
        tokio::spawn(async move { submitter.run(caddy_status, receiver).await });
        let handle = HeartbeatEmitterHandle(sender);
        Ok(handle)
    }

    async fn run(self, caddy_status: CaddyStatus, mut receiver: Receiver<HeartbeatEmitterCommand>) {
        info!("Waiting for caddy to generate a TLS certificate before emitting heartbeats");
        caddy_status.wait_tls_certificate().await;
        info!("Starting heartbeat generation");

        let provider = loop {
            match self.connect().await {
                Ok(out) => break out,
                Err(e) => {
                    error!("Failed to connect to provider: {e}");
                    sleep(CONNECT_RETRY_INTERVAL).await;
                }
            }
        };

        let manager = HeartbeatManager::new(self.heartbeat_contract_address, &provider);
        let token = NilToken::new(self.token_contract_address, &provider);
        loop {
            match self.approve_nil(&token).await {
                Ok(_) => {
                    info!("NIL payments approved, starting heartbeat loop");
                    break;
                }
                Err(e) => {
                    error!("Failed to approve NIL payments: {e}");
                    sleep(TOKEN_APPROVAL_RETRY_INTERVAL).await;
                }
            };
        }
        let mut ctx = Context::new(self.tick_interval);
        for i in 0_u64.. {
            if i % 5 == 0 {
                self.display_balance(&provider, &token).await;
            }

            ctx.ticker.tick().await;

            if let Err(e) = self.submit_htx(&manager).await {
                error!("Failed to submit HTX: {e}");
            }
            ctx = Self::process_pending_commands(&mut receiver, ctx);
        }
    }

    async fn connect(&self) -> anyhow::Result<impl Provider> {
        let ws = WsConnect::new(&self.rpc_endpoint).with_max_retries(u32::MAX);
        let provider = ProviderBuilder::new()
            .wallet(self.wallet.clone())
            .with_simple_nonce_management()
            .with_gas_estimation()
            .connect_ws(ws)
            .await?;
        info!("Connected to RPC endpoint");
        Ok(provider)
    }

    async fn approve_nil(&self, contract: &NilTokenInstance<impl Provider>) -> anyhow::Result<()> {
        let allowance = contract.allowance(self.wallet.address(), self.heartbeat_contract_address).call().await?;
        if allowance == Uint::MAX {
            info!("Address already has a configured allowance, ignoring request");
            return Ok(());
        }
        info!("Need to approve payment allowance for heartbeat contract at {}", self.heartbeat_contract_address);

        // Approve an infinite payment allowance since we fully trust the heartbeat manager
        let tx = contract.approve(self.heartbeat_contract_address, Uint::MAX).send().await?.get_receipt().await?;
        info!("Approved payment allowance on tx {}", tx.transaction_hash);
        Ok(())
    }

    async fn display_balance(&self, provider: &impl Provider, token: &NilTokenInstance<impl Provider>) {
        let address = self.wallet.address();
        let eth_balance = match provider.get_balance(address).await {
            Ok(balance) => balance,
            Err(e) => {
                warn!("Failed to get wallet ETH balance: {e}");
                return;
            }
        };
        let nil_balance = match token.balanceOf(address).call().await {
            Ok(balance) => balance,
            Err(e) => {
                warn!("Failed to get wallet NIL balance: {e}");
                return;
            }
        };
        let eth_balance = alloy::primitives::utils::format_ether(eth_balance);
        let nil_balance = alloy::primitives::utils::format_units(nil_balance, NIL_TOKEN_DECIMALS).unwrap_or_default();
        info!("Wallet {address} has {eth_balance} ETH and {nil_balance} NIL");
    }

    async fn submit_htx(&self, router: &HeartbeatManagerInstance<impl Provider>) -> anyhow::Result<()> {
        let htx = Htx::Nillion(NillionHtx::V1(NillionHtxV1 {
            workload_id: WorkloadId { current: self.workload_id.to_string() },
            workload_measurement: WorkloadMeasurement {
                url: self.attestation_url.clone(),
                artifacts_version: self.nilcc_version.clone(),
                cpus: self.cpu_count,
                gpus: self.gpu_count,
                docker_compose_hash: self.docker_compose_hash,
            },
            builder_measurement: BuilderMeasurement { url: self.measurement_hash_url.clone() },
        }));
        // Use the current block - 1 for the snapshot id
        let snapshot_id = router.provider().get_block_number().await.context("failed to get block number")?;
        let snapshot_id = snapshot_id.saturating_sub(1);
        let htx = htx.to_bytes()?;
        let call = router.submitHeartbeat(htx.into(), snapshot_id);
        let pending_tx = call.send().await?;
        let receipt = pending_tx.get_receipt().await?;
        let tx_hash = receipt.transaction_hash;
        let status = if receipt.status() { "success" } else { "failure" };
        info!("HTX submitted in transaction {tx_hash} with status {status}");
        Ok(())
    }

    fn process_pending_commands(receiver: &mut Receiver<HeartbeatEmitterCommand>, mut ctx: Context) -> Context {
        while let Ok(command) = receiver.try_recv() {
            match command {
                HeartbeatEmitterCommand::SetInterval(interval) => {
                    ctx = Context::new(interval);
                    // Only tick once after the period, not right away
                    ctx.ticker.reset();
                }
            }
        }
        ctx
    }
}

#[must_use]
pub(crate) struct HeartbeatEmitterHandle(Sender<HeartbeatEmitterCommand>);

impl HeartbeatEmitterHandle {
    pub(crate) async fn set_interval(&self, interval: Duration) {
        if self.0.send(HeartbeatEmitterCommand::SetInterval(interval)).await.is_err() {
            error!("Heartbeat emitter channel dropped");
        }
    }
}

pub(crate) enum HeartbeatEmitterCommand {
    SetInterval(Duration),
}

struct Context {
    ticker: Interval,
}

impl Context {
    fn new(tick_interval: Duration) -> Self {
        let mut ticker = interval(tick_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        Self { ticker }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkloadId {
    current: String,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkloadMeasurement {
    url: String,
    artifacts_version: String,
    cpus: u64,
    gpus: u64,
    #[serde_as(as = "Hex")]
    docker_compose_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuilderMeasurement {
    url: String,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NillionHtxV1 {
    workload_id: WorkloadId,
    workload_measurement: WorkloadMeasurement,
    builder_measurement: BuilderMeasurement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "camelCase")]
enum NillionHtx {
    V1(NillionHtxV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "camelCase")]
enum Htx {
    Nillion(NillionHtx),
}

impl Htx {
    fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let json = serde_json::to_value(self)?;
        Ok(serde_json::to_vec(&json)?)
    }
}
