use crate::{
    certs::FetcherError,
    measurement::MeasurementHashError,
    report::{ReportBundle, ReportBundleError, ReportResponse, VmType},
    verify::VerificationError,
};
use anyhow::Context;
use certs::DefaultCertificateFetcher;
use clap::{Args, CommandFactory, Parser, Subcommand, error::ErrorKind};
use measurement::MeasurementGenerator;
use nilcc_artifacts::{
    Artifacts,
    downloader::{ArtifactsDownloader, DownloadError},
    metadata::ArtifactsMetadata,
};
use report::ReportFetcher;
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::exit,
};
use tracing::{error, info, level_filters::LevelFilter};
use verify::ReportVerifier;

mod certs;
mod measurement;
mod report;
mod verify;

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

#[derive(Debug, thiserror::Error)]
enum ValidateError {
    #[error("invalid hex docker compose hash")]
    DockerComposeHash,

    #[error("creating cert cache directories: {0}")]
    CertCacheDirectories(io::Error),

    #[error("fetching report bundle: {0}")]
    ReportBundle(#[from] ReportBundleError),

    #[error(transparent)]
    MeasurementHash(#[from] MeasurementHashError),

    #[error("verifying report: {0}")]
    VerifyReports(#[from] VerificationError),
}

fn decode_compose_hash(input: &str) -> Result<[u8; 32], ValidateError> {
    let mut hash: [u8; 32] = [0; 32];
    hex::decode_to_slice(input, &mut hash).map_err(|_| ValidateError::DockerComposeHash)?;
    Ok(hash)
}

fn validate(args: ValidateArgs) -> Result<ReportMetadata, ValidateError> {
    let ValidateArgs { endpoint, artifact_cache, cert_cache, docker_compose_hash, artifacts_url } = args;
    let docker_compose_hash = decode_compose_hash(&docker_compose_hash)?;
    let fetcher = ReportFetcher::new(artifact_cache, artifacts_url);
    let bundle = fetcher.fetch_report(&endpoint)?;
    let ReportBundle {
        cpu_count,
        ovmf_path,
        initrd_path,
        kernel_path,
        filesystem_root_hash,
        tls_fingerprint,
        nilcc_version,
        metadata,
        ..
    } = bundle;

    let measurement = MeasurementGenerator {
        vcpus: cpu_count,
        ovmf: ovmf_path,
        kernel: kernel_path,
        initrd: initrd_path,
        docker_compose_hash,
        filesystem_root_hash,
        kernel_args: metadata.cvm.cmdline.clone(),
    }
    .generate()?;
    let fetcher = DefaultCertificateFetcher::new(cert_cache).map_err(ValidateError::CertCacheDirectories)?;
    let verifier = ReportVerifier::new(Box::new(fetcher));
    verifier.verify_report(bundle.report, &measurement)?;

    let meta = ReportMetadata {
        measurement_hash: hex::encode(measurement),
        tls_fingerprint,
        artifacts: ReportArtifacts { version: nilcc_version, metadata },
    };
    Ok(meta)
}

fn compute_measurement_hash(args: MeasurementHashArgs) -> anyhow::Result<()> {
    let MeasurementHashArgs { artifact_cache, artifacts_url, vm_type, cpus, docker_compose_hash, nilcc_version } = args;
    let download_path = artifact_cache.join(&nilcc_version);
    let docker_compose_hash = decode_compose_hash(&docker_compose_hash)?;
    let downloader = ArtifactsDownloader::new(nilcc_version.clone(), vec![vm_type.into()])
        .without_disk_images()
        .without_artifact_overwrite()
        .with_artifacts_url(artifacts_url);
    let runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().context("building tokio runtime")?;
    let artifacts = runtime.block_on(downloader.download(&download_path))?;
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

fn download_artifacts(args: DownloadArtifactsArgs) -> anyhow::Result<()> {
    let DownloadArtifactsArgs { workload_url, artifacts_version, output_directory, artifacts_url } = args;
    let version = match (workload_url, artifacts_version) {
        (Some(workload_url), None) => {
            let report_url = format!("{workload_url}/nilcc/api/v2/report");
            let report: ReportResponse = reqwest::blocking::get(report_url)
                .context("Failed to fetch artifacts version")?
                .json()
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
    let runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().context("building tokio runtime")?;
    let artifacts = runtime.block_on(downloader.download(Path::new(&output_directory)))?;
    let metadata_hash = hex::encode(artifacts.metadata_hash);
    println!("Artifacts downloaded at {output_directory}, metadata hash = {metadata_hash}");
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ErrorCode {
    InvalidDockerComposeHash,
    InvalidTlsFingerprint,
    InvalidArtifacts,
    InvalidReport,
    InvalidAmdCerts,
    Filesystem,
    Request,
    Internal,
}

impl From<ValidateError> for ErrorCode {
    fn from(e: ValidateError) -> Self {
        use ErrorCode::*;
        match e {
            ValidateError::DockerComposeHash => InvalidDockerComposeHash,
            ValidateError::CertCacheDirectories(_) => Filesystem,
            ValidateError::ReportBundle(e) => match e {
                ReportBundleError::TlsFingerprint { .. } => InvalidTlsFingerprint,
                ReportBundleError::HttpClient(_) | ReportBundleError::Tokio(_) => Internal,
                ReportBundleError::FetchAttestation(_)
                | ReportBundleError::NoTlsInfo
                | ReportBundleError::TlsCertificate(_)
                | ReportBundleError::NotHttpsScheme
                | ReportBundleError::InvalidUrl(_)
                | ReportBundleError::MalformedPayload(_) => Request,
                ReportBundleError::DownloadArtifacts(e) => match e {
                    DownloadError::NoParent => Internal,
                    DownloadError::TargetDirectory(_)
                    | DownloadError::TargetFile(_)
                    | DownloadError::ReadRootHash(_) => Filesystem,
                    DownloadError::DecodeRootHash(_) => InvalidArtifacts,
                    DownloadError::Download(_) => Request,
                },
            },
            ValidateError::MeasurementHash(_) => Internal,
            ValidateError::VerifyReports(e) => match e {
                VerificationError::FetchCerts(e) => match e {
                    FetcherError::TurinFmc | FetcherError::ZeroHardwareId => InvalidReport,
                    FetcherError::ReadCachedCert(_) | FetcherError::WriteCachedCert(_) => Filesystem,
                    FetcherError::FetchingVcek(_) | FetcherError::FetchingCertChain(_) => Request,
                    FetcherError::ParsingVcek(_) | FetcherError::ParsingCertChain(_) => InvalidAmdCerts,
                },
                VerificationError::CertVerification(_)
                | VerificationError::MalformedCertificate(_)
                | VerificationError::InvalidCertificate(_) => InvalidAmdCerts,
                VerificationError::DetectProcessor(_)
                | VerificationError::InvalidMeasurement { .. }
                | VerificationError::InvalidVcekPubKey
                | VerificationError::MalformedReportSignature
                | VerificationError::InvalidSignature => InvalidReport,
                VerificationError::SerializeReport(_) => Internal,
            },
        }
    }
}

#[derive(Serialize)]
struct ReportMetadata {
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

fn main() {
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
            let (exit_code, result) = match validate(args) {
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
            if let Err(e) = compute_measurement_hash(args) {
                error!("Failed to compute measurement hash: {e:#}");
                exit(1);
            }
        }
        Command::DownloadArtifacts(args) => {
            if let Err(e) = download_artifacts(args) {
                error!("Failed to download artifacts: {e:#}");
                exit(1);
            }
        }
    }
}
