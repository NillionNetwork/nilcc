use anyhow::{anyhow, bail, Context};
use object_store::{aws::AmazonS3, path::Path, ObjectStore};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sev::firmware::guest::AttestationReport;
use std::{fs, path::PathBuf, time::Duration};
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
pub(crate) struct EnvironmentSpec {
    nilcc_version: String,
    vm_type: String,
    cpu_count: u32,
}

pub struct ReportFetcher {
    cache_path: PathBuf,
    s3_client: AmazonS3,
}

impl ReportFetcher {
    pub fn new(cache_path: PathBuf, s3_client: AmazonS3) -> Self {
        Self { cache_path, s3_client }
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
        info!("CVM is running nilcc-version {nilcc_version}, using VM type '{vm_type}' and has {cpu_count} CPUs");

        // Create the cache directory if it doesn't exist already.
        fs::create_dir_all(self.cache_path.join(nilcc_version)).context("creating cache directory")?;

        info!("Downloading artifacts, using {} as cache", self.cache_path.display());
        let artifacts = self.download_artifacts(&environment).context("downloading artifacts")?;
        Ok(ReportBundle { report, cpu_count: *cpu_count, artifacts })
    }

    fn download_artifacts(&self, spec: &EnvironmentSpec) -> anyhow::Result<Artifacts> {
        let version = &spec.nilcc_version;
        let vm_type = &spec.vm_type;
        let ovmf_path = self.download_artifact(version, "vm_images/ovmf/OVMF.fd")?;
        let kernel_path = self.download_artifact(version, &format!("vm_images/kernel/{vm_type}-vmlinuz"))?;
        let initrd_path = self.download_artifact(version, "initramfs/initramfs.cpio.gz")?;
        let filesystem_root_hash_path =
            self.download_artifact(version, &format!("vm_images/cvm-{vm_type}-verity/root-hash"))?;
        let hex_filesystem_root_hash =
            fs::read_to_string(filesystem_root_hash_path).context("reading local copy of filesystem root hash")?;
        let mut filesystem_root_hash: [u8; 32] = [0; 32];
        hex::decode_to_slice(hex_filesystem_root_hash.trim(), &mut filesystem_root_hash)
            .context("decoding filesystem root hash")?;
        Ok(Artifacts { ovmf_path, kernel_path, initrd_path, filesystem_root_hash })
    }

    fn download_artifact(&self, version: &str, artifact_name: &str) -> anyhow::Result<PathBuf> {
        let local_path = self.cache_path.join(version).join(artifact_name);
        if local_path.exists() {
            info!("Not downloading {artifact_name} because it already exists in cache directory");
            return Ok(local_path);
        }
        info!("Need to download {artifact_name}");
        let parent = local_path.parent().ok_or_else(|| anyhow!("path has no parent"))?;
        fs::create_dir_all(parent).context("creating cache directory")?;

        let remote_path = format!("{version}/{artifact_name}");
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().context("building tokio runtime")?;
        let data = runtime
            .block_on(async { self.download_object(&remote_path).await })
            .map_err(|e| anyhow!("failed to download {remote_path}: {e}"))?;
        fs::write(&local_path, data.as_slice()).context("writing data to cache")?;
        Ok(local_path)
    }

    async fn download_object(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let result = self.s3_client.get(&Path::from(path)).await?;
        let bytes = result.bytes().await?;
        Ok(bytes.into())
    }
}

#[derive(Clone, Debug)]
pub struct ReportBundle {
    pub report: AttestationReport,
    pub cpu_count: u32,
    pub artifacts: Artifacts,
}

#[derive(Clone, Debug)]
pub struct Artifacts {
    pub ovmf_path: PathBuf,
    pub kernel_path: PathBuf,
    pub initrd_path: PathBuf,
    pub filesystem_root_hash: [u8; 32],
}
