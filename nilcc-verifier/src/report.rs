use anyhow::{bail, Context};
use clap::ValueEnum;
use nilcc_artifacts::{Artifacts, ArtifactsDownloader, VmTypeArtifacts};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sev::firmware::guest::AttestationReport;
use std::{path::PathBuf, time::Duration};
use tracing::info;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
struct Request {
    #[serde(with = "hex::serde")]
    nonce: [u8; 64],
}

#[derive(Deserialize)]
struct Response {
    report: AttestationReport,
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

    pub fn fetch_report(&self, base_url: &str) -> anyhow::Result<ReportBundle> {
        let request = Request { nonce: rand::random() };
        let http_client = Client::new();
        let url = format!("{base_url}/nilcc/api/v1/report/generate");
        info!("Fetching report from {url}");
        let response: Response = http_client
            .post(url)
            .json(&request)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .context("fetching attestation")?
            .json()
            .context("decoding payload body")?;

        let Response { report, environment } = response;
        if report.report_data.as_slice() != request.nonce {
            bail!(
                "report data is different: sent {}, got {}",
                hex::encode(request.nonce),
                hex::encode(report.report_data)
            );
        }
        let EnvironmentSpec { nilcc_version, vm_type, cpu_count } = &environment;
        info!("CVM is running nilcc-version {nilcc_version}, using VM type '{vm_type:?}' and has {cpu_count} CPUs");

        // Create the cache directory if it doesn't exist already.
        let download_path = self.cache_path.join(nilcc_version);

        info!("Downloading artifacts, using {} as cache", self.cache_path.display());
        let vm_type = (*vm_type).into();
        let downloader = ArtifactsDownloader::new(nilcc_version.clone(), vec![vm_type])
            .without_disk_images()
            .without_artifact_overwrite()
            .with_artifacts_url(self.artifacts_url.clone());
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().context("building tokio runtime")?;
        let artifacts = runtime.block_on(downloader.download(&download_path))?;
        let Artifacts { ovmf_path, initrd_path, mut type_artifacts } = artifacts;
        let VmTypeArtifacts { kernel_path, filesystem_root_hash, .. } =
            type_artifacts.remove(&vm_type).expect("missing vm type artifacts");
        Ok(ReportBundle { report, cpu_count: *cpu_count, ovmf_path, initrd_path, kernel_path, filesystem_root_hash })
    }
}

#[derive(Clone, Debug)]
pub struct ReportBundle {
    pub report: AttestationReport,
    pub cpu_count: u32,
    pub ovmf_path: PathBuf,
    pub initrd_path: PathBuf,
    pub kernel_path: PathBuf,
    pub filesystem_root_hash: [u8; 32],
}
