use anyhow::{anyhow, Context};
use certs::DefaultCertificateFetcher;
use clap::{Args, Parser, Subcommand};
use measurement::MeasurementGenerator;
use report::{Artifacts, ReportFetcher};
use std::{fs::File, io::stdin, path::PathBuf};
use tracing::{error, info};
use verify::ReportVerifier;

mod certs;
mod measurement;
mod report;
mod verify;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run an offline verification using already downloaded input files and hashes.
    Offline(OfflineArgs),

    /// Run an online verification, pulling an attestation report from a CVM running in nilcc.
    Online(OnlineArgs),
}

#[derive(Args)]
struct OfflineArgs {
    /// The path to the report file or '-' for stdin.
    report_path: String,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cert_cache_path().into_os_string())]
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

#[derive(Args)]
struct OnlineArgs {
    /// The public endpoint for the CVM, e.g. `https://example.com`
    endpoint: String,

    /// The path where artifacts will be cached.
    #[clap(short, long, default_value = default_artifact_cache_path().into_os_string())]
    artifact_cache: PathBuf,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cert_cache_path().into_os_string())]
    cert_cache: PathBuf,

    /// Whether to include debug options in kernel command line (e.g. console/earlyprintk/panic).
    #[clap(long)]
    kernel_debug_options: bool,

    /// The docker compose hash that the CVM executes.
    #[clap(long)]
    docker_compose_hash: String,

    /// The base url from which artifacts should be fetched.
    #[clap(long, default_value = default_artifacts_url())]
    artifacts_url: String,
}

fn default_cache_path() -> PathBuf {
    std::env::temp_dir().join("nilcc-verifier-cache")
}

fn default_cert_cache_path() -> PathBuf {
    default_cache_path().join("certs")
}

fn default_artifact_cache_path() -> PathBuf {
    default_cache_path().join("artifacts")
}

fn default_artifacts_url() -> String {
    "https://nilcc.s3.eu-west-1.amazonaws.com".into()
}

fn decode_hash(name: &str, input: &str) -> anyhow::Result<[u8; 32]> {
    let mut hash: [u8; 32] = [0; 32];
    hex::decode_to_slice(input, &mut hash).map_err(|e| anyhow!("invalid {name} hash: {e}"))?;
    Ok(hash)
}

fn run_offline(args: OfflineArgs) -> anyhow::Result<()> {
    let OfflineArgs {
        report_path,
        cert_cache,
        ovmf,
        kernel,
        initrd,
        docker_compose_hash,
        filesystem_root_hash,
        vcpus,
        kernel_debug_options,
    } = args;
    let report = match report_path.as_str() {
        "-" => serde_json::from_reader(stdin()),
        path => serde_json::from_reader(File::open(path).context("opening input file")?),
    }
    .context("parsing report")?;
    let docker_compose_hash = decode_hash("docker compose", &docker_compose_hash)?;
    let filesystem_root_hash = decode_hash("filesystem root hash", &filesystem_root_hash)?;

    let measurement = MeasurementGenerator {
        vcpus,
        ovmf,
        kernel,
        initrd,
        docker_compose_hash,
        filesystem_root_hash,
        kernel_debug_options,
    }
    .generate()?;
    let fetcher = DefaultCertificateFetcher::new(cert_cache).context("creating certificate fetcher")?;
    let verifier = ReportVerifier::new(Box::new(fetcher));
    verifier.verify_report(report, measurement).context("verification failed")?;
    Ok(())
}

fn run_online(args: OnlineArgs) -> anyhow::Result<()> {
    let OnlineArgs { endpoint, artifact_cache, cert_cache, docker_compose_hash, kernel_debug_options, artifacts_url } =
        args;
    let docker_compose_hash = decode_hash("docker compose", &docker_compose_hash)?;
    let fetcher = ReportFetcher::new(artifact_cache, artifacts_url);
    let bundle = fetcher.fetch_report(&endpoint).context("fetching report")?;
    let Artifacts { ovmf_path, kernel_path, initrd_path, filesystem_root_hash } = bundle.artifacts;

    let measurement = MeasurementGenerator {
        vcpus: bundle.cpu_count,
        ovmf: ovmf_path,
        kernel: kernel_path,
        initrd: initrd_path,
        docker_compose_hash,
        filesystem_root_hash,
        kernel_debug_options,
    }
    .generate()?;
    let fetcher = DefaultCertificateFetcher::new(cert_cache).context("creating certificate fetcher")?;
    let verifier = ReportVerifier::new(Box::new(fetcher));
    verifier.verify_report(bundle.report, measurement).context("verification failed")?;
    Ok(())
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Offline(args) => run_offline(args),
        Command::Online(args) => run_online(args),
    }
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
