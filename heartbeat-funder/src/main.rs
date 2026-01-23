use crate::{
    agent::{NilccAgentClient, NilccAgentMonitor, NilccAgentMonitorArgs},
    api::{NilccApiClient, NilccApiMonitor, NilccApiMonitorArgs},
    config::{Config, OtelConfig},
    funder::{Funder, FunderArgs},
};
use anyhow::{Context, bail};
use clap::Parser;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{MetricExporterBuilder, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use std::env;
use tokio::signal::{self, unix::SignalKind};
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::filter::EnvFilter;

mod agent;
mod api;
mod config;
mod funder;
mod metrics;

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

fn is_otel_disabled() -> bool {
    env::var("OTEL_SDK_DISABLED").as_deref() == Ok("true")
}

fn setup_otel(config: OtelConfig) -> anyhow::Result<SdkMeterProvider> {
    let service_name = env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "heartbeat-funder".to_string());
    let mut attributes = vec![KeyValue::new("service.version", env!("CARGO_PKG_VERSION"))];
    for (key, value) in config.resource_attributes {
        attributes.push(KeyValue::new(key, value));
    }
    let resource = Resource::builder().with_service_name(service_name).build();
    let exporter = MetricExporterBuilder::new()
        .with_tonic()
        .with_endpoint(config.endpoint)
        .with_timeout(config.export_timeout)
        .build()
        .context("Failed to build metrics exporter")?;

    let reader = PeriodicReader::builder(exporter).with_interval(config.metrics.export_interval).build();
    let provider = SdkMeterProvider::builder().with_resource(resource).with_reader(reader).build();
    opentelemetry::global::set_meter_provider(provider.clone());
    Ok(provider)
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

    if !config.thresholds.nil.target.is_zero() && config.thresholds.nil.minimum >= config.thresholds.nil.target {
        bail!("NIL minimum funding threshold must be lower than its target");
    }

    let metrics_handle = match is_otel_disabled() {
        true => {
            info!("OTEL metrics disabled via env var");
            None
        }
        false => {
            info!("Exporting OTEL samples to {}", config.otel.endpoint);
            let handle = setup_otel(config.otel).context("Failed to setup OTEL")?;
            Some(handle)
        }
    };

    let funder = Funder::spawn(FunderArgs {
        rpc_endpoint: config.rpc.endpoint,
        signer: config.private_key,
        static_addresses: config.wallets,
        poll_interval: config.intervals.funding,
        thresholds: config.thresholds,
        contracts: config.contracts,
    });
    for agent in config.agents {
        let client = NilccAgentClient::new(agent.url, &agent.token);
        NilccAgentMonitor::spawn(NilccAgentMonitorArgs {
            client,
            poll_interval: config.intervals.agent,
            funder_handle: funder.clone(),
        });
    }
    if let Some(api) = config.api {
        let client = NilccApiClient::new(api.url, &api.token);
        NilccApiMonitor::spawn(NilccApiMonitorArgs {
            client,
            poll_interval: config.intervals.api,
            agent_poll_interval: config.intervals.agent,
            funder_handle: funder.clone(),
        })
    }

    shutdown_signal().await;
    if let Some(handle) = metrics_handle {
        info!("Shutting down metrics exporter");
        let _ = handle.shutdown();
    }
    Ok(())
}
