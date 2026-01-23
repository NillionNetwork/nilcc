use crate::routes::build_router;
use anyhow::Context;
use attestation_verification::{
    DefaultCertificateFetcher, ErrorCode, MeasurementGenerator, ReportBundle, ReportFetcher, ReportResponse,
    ReportVerifier, ValidateError, VmType, report::DefaultArtifactsDownloaderBuilder,
};
use clap::{Args, CommandFactory, Parser, Subcommand, error::ErrorKind};
use nilcc_artifacts::{Artifacts, downloader::ArtifactsDownloader, metadata::ArtifactsMetadata};
use serde::Serialize;
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::exit,
    sync::Arc,
};
use tokio::{
    net::TcpListener,
    signal::{self, unix::SignalKind},
};
use tracing::{error, info, level_filters::LevelFilter};

mod routes;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the integrity of a workloadA.
    Validate(ValidateArgs),

    /// Generate the measurement hash for the given compose hash and artifacts version.
    MeasurementHash(MeasurementHashArgs),

    /// Download the artifacts for a nilcc release version.
    DownloadArtifacts(DownloadArtifactsArgs),

    /// Start an HTTP API that allows validating attestations.
    Serve(ServeArgs),
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
}

#[derive(Args)]
struct ValidateArgs {
    /// The public endpoint for the CVM, e.g. `https://example.com`
    endpoint: String,

    /// The path where artifacts will be cached.
    #[clap(short, long, default_value = default_artifact_cache_path().into_os_string())]
    artifact_cache: PathBuf,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cert_cache_path().into_os_string())]
    cert_cache: PathBuf,

    /// The docker compose hash that the CVM executes.
    #[clap(long)]
    docker_compose_hash: String,

    /// The base url from which artifacts should be fetched.
    #[clap(long, default_value = default_artifacts_url())]
    artifacts_url: String,
}

#[derive(Args)]
struct MeasurementHashArgs {
    /// The path where artifacts will be cached.
    #[clap(short, long, default_value = default_artifact_cache_path().into_os_string())]
    artifact_cache: PathBuf,

    /// The base url from which artifacts should be fetched.
    #[clap(long, default_value = default_artifacts_url())]
    artifacts_url: String,

    /// The type of VM being used.
    #[clap(long)]
    vm_type: VmType,

    /// The number of CPUs being used.
    #[clap(long)]
    cpus: u32,

    /// The docker compose hash that the CVM executes.
    docker_compose_hash: String,

    /// The nilcc artifacts version that's being used.
    nilcc_version: String,
}

#[derive(Args)]
struct DownloadArtifactsArgs {
    /// The workload to use as a base to extract the artifacts version to download.
    #[clap(short, long, group = "version-source")]
    workload_url: Option<String>,

    /// The artifacts version to download.
    #[clap(short, long, group = "version-source")]
    artifacts_version: Option<String>,

    /// The directory where to artifacts are to be downloaded.
    #[clap(short, long)]
    output_directory: Option<String>,

    /// The base url from which artifacts should be fetched.
    #[clap(long, default_value = default_artifacts_url())]
    artifacts_url: String,
}

#[derive(Args)]
struct ServeArgs {
    /// The endpoint to bind to.
    #[clap(short, long, default_value = "0.0.0.0:8080")]
    bind_endpoint: SocketAddr,

    /// The path where artifacts will be cached.
    #[clap(short, long, default_value = default_artifact_cache_path().into_os_string())]
    artifact_cache: PathBuf,

    /// The path where certificates will be cached.
    #[clap(short, long, default_value = default_cert_cache_path().into_os_string())]
    cert_cache: PathBuf,
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

fn decode_compose_hash(input: &str) -> Result<[u8; 32], ValidateError> {
    let mut hash: [u8; 32] = [0; 32];
    hex::decode_to_slice(input, &mut hash).map_err(|_| ValidateError::DockerComposeHash)?;
    Ok(hash)
}

async fn validate(args: ValidateArgs) -> Result<ReportMetadata, ValidateError> {
    let ValidateArgs { endpoint, artifact_cache, cert_cache, docker_compose_hash, artifacts_url } = args;
    let docker_compose_hash = decode_compose_hash(&docker_compose_hash)?;
    let fetcher =
        ReportFetcher::new(artifact_cache.clone(), artifacts_url, Box::new(DefaultArtifactsDownloaderBuilder));
    let bundle = fetcher.fetch_report(&endpoint).await?;
    let ReportBundle { cpu_count, metadata_hash, tls_fingerprint, nilcc_version, metadata, vm_type, .. } = bundle;

    let artifacts_path = artifact_cache.join(&nilcc_version);
    let measurement =
        MeasurementGenerator::new(docker_compose_hash, cpu_count, vm_type.into(), &metadata, &artifacts_path)
            .generate()?;
    let fetcher = DefaultCertificateFetcher::new(cert_cache).map_err(ValidateError::CertCacheDirectories)?;
    let verifier = ReportVerifier::new(Arc::new(fetcher));
    verifier.verify_report(&bundle.report, &measurement).await?;

    let github_actions_build_url = metadata.build.as_ref().map(|b| {
        let id = b.github_action_run_id;
        format!("https://github.com/NillionNetwork/nilcc/actions/runs/{id}")
    });
    let metadata_hash = hex::encode(metadata_hash);
    let meta = ReportMetadata {
        github_actions_build_url,
        measurement_hash: hex::encode(measurement),
        metadata_hash,
        tls_fingerprint,
        artifacts: ReportArtifacts { version: nilcc_version, metadata },
    };
    Ok(meta)
}

async fn compute_measurement_hash(args: MeasurementHashArgs) -> anyhow::Result<()> {
    let MeasurementHashArgs { artifact_cache, artifacts_url, vm_type, cpus, docker_compose_hash, nilcc_version } = args;
    let download_path = artifact_cache.join(&nilcc_version);
    let docker_compose_hash = decode_compose_hash(&docker_compose_hash)?;
    let downloader = ArtifactsDownloader::new(nilcc_version.clone(), vec![vm_type.into()])
        .without_disk_images()
        .without_artifact_overwrite()
        .with_artifacts_url(artifacts_url);
    let artifacts = downloader.download(&download_path).await?;
    let Artifacts { metadata, .. } = artifacts;
    let vm_type_metadata = metadata.cvm.images.resolve(vm_type.into());
    let filesystem_root_hash = vm_type_metadata.verity.root_hash;
    let ovmf_path = download_path.join(&metadata.ovmf.path);
    let initrd_path = download_path.join(&metadata.initrd.path);
    let kernel_path = download_path.join(&vm_type_metadata.kernel.path);
    let measurement = MeasurementGenerator {
        vcpus: cpus,
        ovmf: ovmf_path,
        kernel: kernel_path,
        initrd: initrd_path,
        docker_compose_hash,
        filesystem_root_hash,
        kernel_args: metadata.cvm.cmdline.clone(),
    }
    .generate()?;
    let measurement = hex::encode(&measurement);
    info!("Measurement hash: {measurement}");
    println!("{measurement}");
    Ok(())
}

async fn download_artifacts(args: DownloadArtifactsArgs) -> anyhow::Result<()> {
    let DownloadArtifactsArgs { workload_url, artifacts_version, output_directory, artifacts_url } = args;
    let version = match (workload_url, artifacts_version) {
        (Some(workload_url), None) => {
            let report_url = format!("{workload_url}/nilcc/api/v2/report");
            let report: ReportResponse = reqwest::get(report_url)
                .await
                .context("Failed to fetch artifacts version")?
                .json()
                .await
                .context("Malformed attestation report")?;
            report.environment.nilcc_version
        }
        (None, Some(version)) => version,
        (None, None) => {
            Cli::command()
                .error(ErrorKind::MissingRequiredArgument, "need either a workload URL or an artifact version")
                .exit();
        }
        _ => unreachable!(),
    };
    let output_directory = match output_directory {
        Some(path) => path,
        None => format!("nilcc-{version}"),
    };
    fs::create_dir_all(&output_directory).context("Failed to create output directory")?;
    let downloader = ArtifactsDownloader::new(version, vec![VmType::Cpu.into(), VmType::Gpu.into()])
        .with_artifacts_url(artifacts_url);
    println!("Downloading artifacts...");
    let artifacts = downloader.download(Path::new(&output_directory)).await?;
    let metadata_hash = hex::encode(artifacts.metadata_hash);
    println!("Artifacts downloaded at {output_directory}, metadata hash = {metadata_hash}");
    Ok(())
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let ServeArgs { bind_endpoint, artifact_cache, cert_cache } = args;
    let router = build_router(cert_cache, artifact_cache).context("building HTTP router")?;
    let listener = TcpListener::bind(bind_endpoint).await.expect("failed to bind");
    info!("Launching server in {bind_endpoint}");
    axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
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

#[derive(Serialize)]
struct ReportMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    github_actions_build_url: Option<String>,
    metadata_hash: String,
    measurement_hash: String,
    tls_fingerprint: String,
    artifacts: ReportArtifacts,
}

#[derive(Serialize)]
struct ReportArtifacts {
    version: String,
    #[serde(flatten)]
    metadata: ArtifactsMetadata,
}

#[derive(Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
enum ValidateResult {
    Success { metadata: Box<ReportMetadata> },
    Failure { error_code: ErrorCode, message: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let default_log_level = match cli.verbose {
        true => LevelFilter::INFO,
        false => LevelFilter::ERROR,
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::filter::EnvFilter::builder()
                .with_default_directive(default_log_level.into())
                .from_env_lossy(),
        )
        .init();

    match cli.command {
        Command::Validate(args) => {
            let (exit_code, result) = match validate(args).await {
                Ok(metadata) => (0, ValidateResult::Success { metadata: metadata.into() }),
                Err(e) => {
                    let message = e.to_string();
                    (1, ValidateResult::Failure { error_code: e.into(), message })
                }
            };
            println!("{}", serde_json::to_string(&result).expect("failed to serialize"));
            exit(exit_code);
        }
        Command::MeasurementHash(args) => {
            if let Err(e) = compute_measurement_hash(args).await {
                error!("Failed to compute measurement hash: {e:#}");
                exit(1);
            }
        }
        Command::DownloadArtifacts(args) => {
            if let Err(e) = download_artifacts(args).await {
                error!("Failed to download artifacts: {e:#}");
                exit(1);
            }
        }
        Command::Serve(args) => {
            if let Err(e) = serve(args).await {
                error!("Failed to serve API: {e:#}");
                exit(1);
            }
        }
    }
}
