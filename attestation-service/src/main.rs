use attestation_service::{
    report::request_hardware_report,
    routes::build_router,
    verify::{Processor, ReportVerifier},
};
use clap::Parser;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Parser)]
struct Cli {
    /// The endpoint to bind to.
    #[clap(short, long, default_value = "0.0.0.0:8080")]
    bind_endpoint: SocketAddr,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let Cli { bind_endpoint } = Cli::parse();

    info!("Getting report to use during initial verification");
    let report = request_hardware_report(rand::random()).expect("failed to get hardware report");

    info!("Verifying report");
    ReportVerifier::default()
        // TODO make configurable
        .with_processor(Processor::Genoa)
        .verify_report(report)
        .await
        .expect("failed to fetch certs");
    info!("Verification succeeded");

    let router = build_router();
    info!("Running server on {bind_endpoint}");
    let listener = TcpListener::bind(bind_endpoint).await.expect("failed to bind");
    axum::serve(listener, router).await.expect("failed to run");
}
