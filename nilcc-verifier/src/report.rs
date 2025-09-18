use clap::ValueEnum;
use nilcc_artifacts::{
    downloader::{ArtifactsDownloader, DownloadError},
    metadata::ArtifactsMetadata,
    Artifacts,
};
use reqwest::{blocking::ClientBuilder, tls::TlsInfo, Url};
use serde::Deserialize;
use sev::firmware::guest::AttestationReport;
use sha2::{Digest, Sha256};
use std::{io, path::PathBuf, time::Duration};
use tracing::info;
use x509_parser::parse_x509_certificate;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct ReportResponse {
    report: attestation_report::v1::AttestationReport,
    environment: EnvironmentSpec,
}

#[derive(Deserialize)]
pub struct EnvironmentSpec {
    pub nilcc_version: String,
    pub vm_type: VmType,
    pub cpu_count: u32,
}

#[derive(Copy, Clone, Debug, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum VmType {
    Gpu,
    Cpu,
}

impl From<VmType> for nilcc_artifacts::VmType {
    fn from(vm_type: VmType) -> Self {
        match vm_type {
            VmType::Gpu => Self::Gpu,
            VmType::Cpu => Self::Cpu,
        }
    }
}

pub struct ReportFetcher {
    cache_path: PathBuf,
    artifacts_url: String,
}

impl ReportFetcher {
    pub fn new(cache_path: PathBuf, artifacts_url: String) -> Self {
        Self { cache_path, artifacts_url }
    }

    pub fn fetch_report(&self, base_url: &str) -> Result<ReportBundle, ReportBundleError> {
        let http_client = ClientBuilder::default().tls_info(true).build().map_err(ReportBundleError::HttpClient)?;
        let mut url: Url = base_url.parse()?;
        if url.scheme() != "https" {
            return Err(ReportBundleError::NotHttpsScheme);
        }
        url.set_path("/nilcc/api/v1/report");
        url.set_query(None);

        info!("Fetching report from {url}");
        let response =
            http_client.get(url).timeout(REQUEST_TIMEOUT).send().map_err(ReportBundleError::FetchAttestation)?;

        let info = response.extensions().get::<TlsInfo>().ok_or(ReportBundleError::NoTlsInfo)?;
        let cert = info.peer_certificate().ok_or(ReportBundleError::NoTlsInfo)?;
        let (_, cert) = parse_x509_certificate(cert).map_err(ReportBundleError::TlsCertificate)?;
        let pubkey = cert.tbs_certificate.subject_pki;
        let cert_fingerprint = Sha256::digest(pubkey.raw);
        let mut expected_report_data: [u8; 64] = [0; 64];
        expected_report_data[1..33].copy_from_slice(&cert_fingerprint);

        let ReportResponse { report, environment } = response.json().map_err(ReportBundleError::MalformedPayload)?;
        let report = AttestationReport::from(report);
        if report.report_data.as_slice() != expected_report_data {
            return Err(ReportBundleError::TlsFingerprint {
                expected: hex::encode(expected_report_data),
                actual: hex::encode(report.report_data),
            });
        }
        info!("Report contains expected TLS fingerprint: {}", hex::encode(cert_fingerprint));

        let EnvironmentSpec { nilcc_version, vm_type, cpu_count } = environment;
        info!("CVM is running nilcc-version {nilcc_version}, using VM type '{vm_type:?}' and has {cpu_count} CPUs");

        // Create the cache directory if it doesn't exist already.
        let download_path = self.cache_path.join(&nilcc_version);

        info!("Downloading artifacts, using {} as cache", self.cache_path.display());
        let vm_type = vm_type.into();
        let downloader = ArtifactsDownloader::new(nilcc_version.clone(), vec![vm_type])
            .without_disk_images()
            .without_artifact_overwrite()
            .with_artifacts_url(self.artifacts_url.clone());
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(ReportBundleError::Tokio)?;
        let artifacts = runtime.block_on(downloader.download(&download_path))?;
        let Artifacts { ovmf_path, initrd_path, metadata, .. } = artifacts;
        let vm_type_metadata = metadata.cvm.images.resolve(vm_type);
        let filesystem_root_hash = vm_type_metadata.verity.root_hash;
        let kernel_path = download_path.join(&vm_type_metadata.kernel.path);
        Ok(ReportBundle {
            report,
            metadata,
            cpu_count,
            ovmf_path,
            initrd_path,
            kernel_path,
            filesystem_root_hash,
            tls_fingerprint: hex::encode(cert_fingerprint),
            nilcc_version,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReportBundleError {
    #[error("failed to create http client: {0}")]
    HttpClient(reqwest::Error),

    #[error("failed to parse URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("failed to fetch attestation: {0}")]
    FetchAttestation(reqwest::Error),

    #[error("workload URL does not use https scheme")]
    NotHttpsScheme,

    #[error("TLS information missing")]
    NoTlsInfo,

    #[error("invalid TLS certificate: {0}")]
    TlsCertificate(nom::Err<x509_parser::error::X509Error>),

    #[error("invalid TLS fingerprint, expected {expected}, got {actual}")]
    TlsFingerprint { expected: String, actual: String },

    #[error("malformed JSON payload: {0}")]
    MalformedPayload(reqwest::Error),

    #[error("failed to create tokio runtime: {0}")]
    Tokio(io::Error),

    #[error("failed to download artifacts: {0}")]
    DownloadArtifacts(#[from] DownloadError),
}

#[derive(Clone, Debug)]
pub struct ReportBundle {
    pub report: AttestationReport,
    pub metadata: ArtifactsMetadata,
    pub cpu_count: u32,
    pub ovmf_path: PathBuf,
    pub initrd_path: PathBuf,
    pub kernel_path: PathBuf,
    pub filesystem_root_hash: [u8; 32],
    pub tls_fingerprint: String,
    pub nilcc_version: String,
}
