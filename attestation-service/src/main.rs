use attestation_service::{
    certs::{CertFetchPolicy, DefaultCertificateFetcher},
    config::Config,
    report::request_hardware_report,
    routes::build_router,
    verify::ReportVerifier,
};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::{error, info};

#[derive(Parser)]
struct Cli {
    /// The path to the config file.
    #[clap(short, long)]
    config_path: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let Cli { config_path } = Cli::parse();
    let config = match Config::load(config_path.as_deref()) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load config: {e:#}");
            std::process::exit(1);
        }
    };

    if config.attestation.verify_on_startup {
        info!("Getting report to use during initial verification");
        let report = request_hardware_report(rand::random()).expect("failed to get hardware report");

        info!("Verifying report");
        let fetcher = DefaultCertificateFetcher::new(CertFetchPolicy::PreferHardware);
        let mut verifier = ReportVerifier::new(Box::new(fetcher));
        if let Some(processor) = config.attestation.processor {
            info!("Forcing processor to be {processor:?}");
            verifier = verifier.with_processor(processor);
        }
        verifier.verify_report(report).await.expect("failed to fetch certs");
        info!("Verification succeeded");
    }

    let bind_endpoint = &config.server.bind_endpoint;
    info!("Running server on {bind_endpoint}");

    let router = build_router();
    let listener = TcpListener::bind(bind_endpoint).await.expect("failed to bind");
    axum::serve(listener, router).await.expect("failed to run");
}
