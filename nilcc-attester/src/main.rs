use anyhow::bail;
use axum::http;
use clap::Parser;
use nilcc_attester::{
    cert::CertFetcher,
    config::{Config, VmType},
    report::{GpuReportConfig, HardwareReporter},
    routes::{build_router, AppState},
};
use std::{process::exit, sync::Arc, time::Duration};
use tokio::{net::TcpListener, signal, time::sleep};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

const MAX_REPORTER_RETRIES: usize = 20;
const REPORTER_RETRY_DELAY: Duration = Duration::from_secs(10);

#[derive(Parser)]
struct Cli {
    /// The path to the config file.
    #[clap(short, long)]
    config_path: Option<String>,
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Received shutdown signal");
}

async fn build_reporter(gpu_config: GpuReportConfig, fetcher: CertFetcher) -> anyhow::Result<HardwareReporter> {
    for _ in 0..MAX_REPORTER_RETRIES {
        match HardwareReporter::new(gpu_config.clone(), fetcher.clone()).await {
            Ok(reporter) => return Ok(reporter),
            Err(e) => {
                warn!("Failed to build hardware reporter: {e:#}");
                sleep(REPORTER_RETRY_DELAY).await
            }
        }
    }
    bail!("Exhausted {MAX_REPORTER_RETRIES} attempts to build hardware reporter")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let Cli { config_path } = Cli::parse();
    let config = match Config::load(config_path.as_deref()) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load config: {e:#}");
            exit(1);
        }
    };

    let bind_endpoint = &config.server.bind_endpoint;
    info!("Running server on {bind_endpoint}");
    let gpu_config = match &config.vm_type {
        VmType::Cpu => GpuReportConfig::Disabled,
        VmType::Gpu => GpuReportConfig::Enabled { attester_path: config.gpu_attester_path },
    };
    let fetcher = CertFetcher { proxy_endpoint: config.proxy_endpoint, server_name: config.attestation_domain };
    let reporter = build_reporter(gpu_config, fetcher).await.expect("Failed to initialize hardware reporter");
    let reporter = Arc::new(reporter);
    let state =
        AppState { nilcc_version: config.nilcc_version, vm_type: config.vm_type, cpu_count: num_cpus::get(), reporter };
    let listener = TcpListener::bind(bind_endpoint).await.expect("failed to bind");
    let cors = CorsLayer::new()
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([http::header::CONTENT_TYPE])
        .allow_origin(tower_http::cors::Any);
    let router = build_router(state).layer(cors);
    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await.expect("failed to run");
}
