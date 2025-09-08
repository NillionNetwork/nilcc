use crate::cert::CertFetcher;
use anyhow::{bail, Context};
use sev::firmware::guest::{AttestationReport, Firmware};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{process::Command, sync::Mutex, time::sleep};
use tracing::{debug, error, info};

const VMPL: u32 = 1;
const CERT_FINGERPRINT_INTERVAL: Duration = Duration::from_secs(30);

pub struct HardwareReporter {
    inner: Arc<Mutex<Inner>>,
}

impl HardwareReporter {
    pub async fn new(gpu: GpuReportConfig, cert_fetcher: CertFetcher) -> anyhow::Result<Self> {
        let fingerprint = cert_fetcher.fetch_fingerprint().await.context("Failed to fetch cert fingerpring")?;
        let inner = Inner {
            hardware: Self::fetch_hardware_report(&fingerprint).context("Failed to fetch hardware report")?,
            gpu_token: Self::fetch_gpu_report(&fingerprint, &gpu).await.context("Failed to fetch GPU report")?,
        };
        let inner = Arc::new(Mutex::new(inner));
        Worker::spawn(gpu, cert_fetcher, fingerprint, inner.clone());
        Ok(Self { inner })
    }

    pub async fn reports(&self) -> (Arc<AttestationReport>, Option<String>) {
        let inner = self.inner.lock().await;
        (inner.hardware.clone(), inner.gpu_token.clone())
    }

    fn fetch_hardware_report(fingerprint: &[u8; 32]) -> anyhow::Result<Arc<AttestationReport>> {
        let mut data: [u8; 64] = [0; 64];
        // Version, bump if changed
        data[0] = 0;
        // Copy over the cert fingerprint
        data[1..33].copy_from_slice(fingerprint);

        info!("Generating hardware report using nonce {}", hex::encode(data));
        let mut fw = Firmware::open().context("unable to open /dev/sev-guest")?;
        let raw_report = fw.get_report(None, Some(data), Some(VMPL)).context("unable to fetch attestation report")?;
        let report = AttestationReport::from_bytes(&raw_report)?;
        Ok(Arc::new(report))
    }

    async fn fetch_gpu_report(fingerprint: &[u8; 32], gpu: &GpuReportConfig) -> anyhow::Result<Option<String>> {
        let gpu_attester_path = match gpu {
            GpuReportConfig::Enabled { attester_path } => attester_path,
            GpuReportConfig::Disabled => return Ok(None),
        };
        let nonce = hex::encode(fingerprint);
        info!("Generating GPU report using nonce {nonce}");

        let output = Command::new(gpu_attester_path)
            .arg(nonce)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("failed to invoke GPU attester")?;
        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).trim().into()))
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("could not generate GPU report. stderr = '{stderr}', stdout = '{stdout}'")
        }
    }
}

pub enum GpuReportConfig {
    Enabled { attester_path: PathBuf },
    Disabled,
}

struct Inner {
    hardware: Arc<AttestationReport>,
    gpu_token: Option<String>,
}

struct Worker {
    gpu: GpuReportConfig,
    cert_fetcher: CertFetcher,
    fingerprint: [u8; 32],
    inner: Arc<Mutex<Inner>>,
}

impl Worker {
    fn spawn(gpu: GpuReportConfig, cert_fetcher: CertFetcher, fingerprint: [u8; 32], inner: Arc<Mutex<Inner>>) {
        let worker = Self { gpu, cert_fetcher, fingerprint, inner };
        tokio::spawn(async move {
            worker.run().await;
        });
    }

    async fn run(mut self) {
        loop {
            sleep(CERT_FINGERPRINT_INTERVAL).await;

            match self.fetch().await {
                Ok(()) => {}
                Err(e) => {
                    error!("Failed to fetch: {e:#}");
                }
            };
        }
    }

    async fn fetch(&mut self) -> anyhow::Result<()> {
        let fingerprint = self.cert_fetcher.fetch_fingerprint().await.context("Failed to fetch fingerprint")?;
        if fingerprint == self.fingerprint {
            debug!("Cert fingerprint hasn't changed");
            return Ok(());
        }
        info!(
            "Certificate fingerpring changed from {} to {}, re-generating reports",
            hex::encode(self.fingerprint),
            hex::encode(fingerprint)
        );
        let hardware =
            HardwareReporter::fetch_hardware_report(&fingerprint).context("Failed to fetch hardware report")?;
        let gpu_token =
            HardwareReporter::fetch_gpu_report(&fingerprint, &self.gpu).await.context("Failed to fetch GPU report")?;
        self.fingerprint = fingerprint;
        *self.inner.lock().await = Inner { hardware, gpu_token };
        Ok(())
    }
}
