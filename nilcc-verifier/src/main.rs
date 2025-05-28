use anyhow::Context;
use certs::DefaultCertificateFetcher;
use clap::Parser;
use std::{fs::File, io::stdin, path::PathBuf};
use tracing::{error, info};
use verify::{Processor, ReportVerifier};

mod certs;
mod verify;

#[derive(Parser)]
struct Cli {
    /// The path to the report file or '-' for stdin.
    report_path: String,

    /// The processor the report was generated in.
    #[clap(short, long)]
    processor: Option<Processor>,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cache_path().into_os_string())]
    cache_path: PathBuf,
}

fn default_cache_path() -> PathBuf {
    std::env::temp_dir().join("nilcc-verifier-cache")
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let report = match cli.report_path.as_str() {
        "-" => serde_json::from_reader(stdin()),
        path => serde_json::from_reader(File::open(path).context("opening input file")?),
    }
    .context("parsing report")?;

    let fetcher = DefaultCertificateFetcher::new(cli.cache_path).context("creating certificate fetcher")?;
    let mut verifier = ReportVerifier::new(Box::new(fetcher));
    if let Some(processor) = cli.processor {
        verifier = verifier.with_processor(processor);
    }
    verifier.verify_report(report).context("verification failed")?;
    Ok(())
}

fn main() {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => {
            info!("Verification successful");
        }
        Err(e) => {
            error!("Failed to run: {e:#}");
            std::process::exit(1);
        }
    }
}
