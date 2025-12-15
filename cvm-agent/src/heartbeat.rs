use crate::{heartbeat::NilAVRouter::NilAVRouterInstance, monitors::caddy::CaddyStatus};
use alloy::{primitives::Address, providers::ProviderBuilder, signers::local::PrivateKeySigner, sol_types::sol};
use alloy_provider::{Provider, WsConnect};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use std::time::Duration;
use tokio::time::{MissedTickBehavior, interval, sleep};
use tracing::{error, info, warn};
use uuid::Uuid;

sol! {
    #[sol(rpc)]
    contract NilAVRouter {
        function submitHTX(bytes calldata rawHTX) external returns (bytes32 htxId);
    }
}

const ATTESTATION_PATH: &str = "/nilcc/api/v2/report";
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_secs(10);

pub(crate) struct HeartbeatEmitterArgs {
    pub(crate) workload_id: Uuid,
    pub(crate) workload_domain: String,
    pub(crate) rpc_endpoint: String,
    pub(crate) contract_address: String,
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
    contract_address: Address,
    nilcc_version: String,
    docker_compose_hash: [u8; 32],
    tick_interval: Duration,
    measurement_hash_url: String,
    cpu_count: u64,
    gpu_count: u64,
}

impl HeartbeatEmitter {
    pub(crate) async fn spawn(args: HeartbeatEmitterArgs) -> anyhow::Result<()> {
        let HeartbeatEmitterArgs {
            workload_id,
            workload_domain,
            rpc_endpoint,
            contract_address,
            wallet_private_key,
            nilcc_version,
            docker_compose_hash,
            tick_interval,
            measurement_hash_url,
            cpu_count,
            gpu_count,
            caddy_status,
        } = args;
        let contract_address: Address = contract_address.parse().context("Invalid contract address")?;
        let attestation_url = format!("https://{workload_domain}{ATTESTATION_PATH}");
        let wallet = PrivateKeySigner::from_slice(&wallet_private_key).context("Invalid wallet private key")?;
        info!("Starting heartbeat emitter using wallet {}", wallet.address());

        let submitter = Self {
            workload_id,
            attestation_url,
            rpc_endpoint,
            wallet,
            contract_address,
            nilcc_version,
            docker_compose_hash,
            tick_interval,
            measurement_hash_url,
            cpu_count,
            gpu_count,
        };
        tokio::spawn(async move { submitter.run(caddy_status).await });
        Ok(())
    }

    async fn run(self, caddy_status: CaddyStatus) {
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

        let router = NilAVRouter::new(self.contract_address, &provider);
        let mut ticker = interval(self.tick_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // reset immediately so we start by ticking
        ticker.reset_immediately();
        for i in 0_u64.. {
            if i % 5 == 0 {
                self.display_balance(&provider).await;
            }

            ticker.tick().await;

            if let Err(e) = self.submit_htx(&router).await {
                error!("Failed to submit HTX: {e}");
            }
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

    async fn display_balance(&self, provider: &impl Provider) {
        let address = self.wallet.address();
        let balance = match provider.get_balance(address).await {
            Ok(balance) => balance,
            Err(e) => {
                warn!("Failed to get wallet balance: {e}");
                return;
            }
        };
        let balance = alloy::primitives::utils::format_ether(balance);
        info!("Wallet {address} has {balance} ETH");
    }

    async fn submit_htx(&self, router: &NilAVRouterInstance<impl Provider>) -> anyhow::Result<()> {
        let htx = VersionedHtx::V1(Htx {
            workload_id: WorkloadId { current: self.workload_id.to_string() },
            nilcc_measurement: NilCcMeasurement {
                url: self.attestation_url.clone(),
                nilcc_version: self.nilcc_version.clone(),
                cpu_count: self.cpu_count,
                gpus: self.gpu_count,
                docker_compose_hash: self.docker_compose_hash,
            },
            builder_measurement: BuilderMeasurement { url: self.measurement_hash_url.clone() },
        });
        let htx = htx.to_bytes()?;
        let call = router.submitHTX(htx.into());
        let pending_tx = call.send().await?;
        let receipt = pending_tx.get_receipt().await?;
        let tx_hash = receipt.transaction_hash;
        let status = if receipt.status() { "success" } else { "failure" };
        info!("HTX submitted in transaction {tx_hash} with status {status}");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkloadId {
    current: String,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NilCcMeasurement {
    url: String,

    #[serde(rename = "nilcc_version")]
    nilcc_version: String,

    #[serde(rename = "cpu_count")]
    cpu_count: u64,

    #[serde(rename = "GPUs")]
    gpus: u64,

    #[serde_as(as = "Hex")]
    docker_compose_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuilderMeasurement {
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Htx {
    workload_id: WorkloadId,

    #[serde(rename = "nilCC_measurement")]
    nilcc_measurement: NilCcMeasurement,

    builder_measurement: BuilderMeasurement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", rename_all = "camelCase")]
enum VersionedHtx {
    V1(Htx),
}

impl VersionedHtx {
    fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        // Go through a `serde_json::Value` first to have consistent key ordering
        let json = serde_json::to_value(self)?;
        Ok(serde_json::to_vec(&json)?)
    }
}
