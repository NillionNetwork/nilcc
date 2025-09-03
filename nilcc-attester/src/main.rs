use anyhow::Context;
use axum::http;
use clap::Parser;
use nilcc_attester::{
    cert::CertFetcher,
    config::{Config, VmType},
    report::HardwareReporter,
    routes::{build_router, AppState},
};
use sev::firmware::guest::AttestationReport;
use std::{process::exit, sync::Arc};
use tokio::{net::TcpListener, signal};
use tower_http::cors::CorsLayer;
use tracing::{error, info};

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

async fn generate_report(config: &Config) -> anyhow::Result<(Arc<AttestationReport>, Option<String>)> {
    let cert_fingerprint =
        CertFetcher { proxy_endpoint: config.proxy_endpoint.clone(), server_name: config.attestation_domain.clone() }
            .fetch_fingerprint()
            .await
            .expect("Failed to fetch certificate");
    let reporter = Arc::new(HardwareReporter::new(config.gpu_attester_path.clone()));

    let mut report_data: [u8; 64] = [0; 64];
    // Version, bump if changed
    report_data[0] = 0;
    // Copy over the cert fingerprint
    report_data[1..33].copy_from_slice(&cert_fingerprint);

    let report = reporter.hardware_report(report_data)?;
    let gpu_token = match config.vm_type {
        VmType::Cpu => None,
        VmType::Gpu => Some(reporter.gpu_report(cert_fingerprint).await.context("Failed to get GPU report")?),
    };
    Ok((Arc::new(report), gpu_token))
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
    let (hardware_report, gpu_token) = generate_report(&config).await.expect("Failed to generate report");

    let state = AppState {
        nilcc_version: config.nilcc_version,
        vm_type: config.vm_type,
        cpu_count: num_cpus::get(),
        hardware_report,
        gpu_token,
    };

    let listener = TcpListener::bind(bind_endpoint).await.expect("failed to bind");
    let cors = CorsLayer::new()
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([http::header::CONTENT_TYPE])
        .allow_origin(tower_http::cors::Any);
    let router = build_router(state).layer(cors);
    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await.expect("failed to run");
}
