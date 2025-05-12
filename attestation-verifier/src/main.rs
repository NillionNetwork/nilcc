use anyhow::Context;
use attestation_service::{
    certs::DefaultCertificateFetcher,
    verify::{Processor, ReportVerifier},
};
use clap::Parser;
use std::{fs::File, io::stdin};
use tracing::{error, info};

#[derive(Parser)]
struct Cli {
    /// The path to the report file or '-' for stdin.
    report_path: String,

    /// The processor the report was generated in.
    #[clap(short, long)]
    processor: Option<Processor>,
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let report = match cli.report_path.as_str() {
        "-" => serde_json::from_reader(stdin()),
        path => serde_json::from_reader(File::open(path).context("opening input file")?),
    }
    .context("parsing report")?;
    let mut verifier = ReportVerifier::new(Box::new(DefaultCertificateFetcher));
    if let Some(processor) = cli.processor {
        verifier = verifier.with_processor(processor);
    }
    verifier.verify_report(report).await.context("verification failed")?;
    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => {
            info!("Verification successful");
        }
        Err(e) => {
            error!("Failed to run: {e:#}");
            std::process::exit(1);
        }
    }
}
