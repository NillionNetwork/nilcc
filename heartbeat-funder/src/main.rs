use crate::{
    agent::{NilccAgentClient, NilccAgentMonitor, NilccAgentMonitorArgs},
    config::Config,
    funder::{Funder, FunderArgs},
};
use anyhow::bail;
use clap::Parser;
use tokio::signal::{self, unix::SignalKind};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::filter::EnvFilter;

mod agent;
mod config;
mod funder;

/// Heartbeat funder allows monitoring and funding wallets that are used to emit heartbeats in
/// nilcc workloads.
#[derive(Parser)]
struct Cli {
    /// The path to the config file.
    #[clap(short, long)]
    config_path: Option<String>,
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install ctrl-c handler");
    };

    let terminate = async {
        signal::unix::signal(SignalKind::terminate()).expect("failed to install signal handler").recv().await;
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("Received ctrl-c");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::builder().with_default_directive(LevelFilter::INFO.into()).from_env_lossy())
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config_path.as_deref())?;
    if config.thresholds.eth.minimum >= config.thresholds.eth.target {
        bail!("ETH minimum funding threshold must be lower than its target");
    }

    let funder = Funder::spawn(FunderArgs {
        rpc_endpoint: config.rpc.endpoint,
        signer: config.private_key,
        static_addresses: config.wallets,
        poll_interval: config.intervals.funding,
        thresholds: config.thresholds,
    });
    for agent in config.agents {
        let client = NilccAgentClient::new(agent.url, &agent.token);
        NilccAgentMonitor::spawn(NilccAgentMonitorArgs {
            client,
            poll_interval: config.intervals.agent,
            funder_handle: funder.clone(),
        });
    }

    shutdown_signal().await;
    Ok(())
}
