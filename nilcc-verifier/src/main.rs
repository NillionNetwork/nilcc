use anyhow::Context;
use certs::DefaultCertificateFetcher;
use clap::Parser;
use measurement::MeasurementGenerator;
use std::{fs::File, io::stdin, path::PathBuf};
use tracing::{error, info};
use verify::{Processor, ReportVerifier};

mod certs;
mod measurement;
mod verify;

#[derive(Parser)]
struct Cli {
    /// The path to the report file or '-' for stdin.
    report_path: String,

    /// The processor the report was generated in.
    #[clap(long)]
    processor: Option<Processor>,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cache_path().into_os_string())]
    cert_cache: PathBuf,

    /// The path to the OVMF file used by the CVM.
    #[clap(long)]
    ovmf: PathBuf,

    /// The path to the kernel used by the CVM.
    #[clap(long)]
    kernel: PathBuf,

    /// The path to the initrd used by the CVM.
    #[clap(long)]
    initrd: PathBuf,

    /// The docker compose hash that the CVM executes.
    #[clap(long)]
    docker_compose_hash: String,

    /// The root hash for the filesystem used when booting.
    #[clap(long)]
    filesystem_root_hash: String,

    /// The number of VCPUs the VM has.
    #[clap(long)]
    vcpus: u32,

    /// Whether to include debug options in kernel command line (e.g. console/earlyprintk/panic).
    #[clap(long)]
    kernel_debug_options: bool,
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

    let measurement = MeasurementGenerator {
        vcpus: cli.vcpus,
        ovmf: cli.ovmf,
        kernel: cli.kernel,
        initrd: cli.initrd,
        docker_compose_hash: cli.docker_compose_hash,
        filesystem_root_hash: cli.filesystem_root_hash,
        kernel_debug_options: cli.kernel_debug_options,
    }
    .generate()?;
    let fetcher = DefaultCertificateFetcher::new(cli.cert_cache).context("creating certificate fetcher")?;
    let mut verifier = ReportVerifier::new(Box::new(fetcher));
    if let Some(processor) = cli.processor {
        verifier = verifier.with_processor(processor);
    }
    verifier.verify_report(report, measurement).context("verification failed")?;
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
