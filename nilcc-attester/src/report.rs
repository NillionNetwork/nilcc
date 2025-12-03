use crate::cert::CertFetcher;
use anyhow::{Context, bail};
use sev::{
    firmware::guest::{AttestationReport, Firmware},
    parser::ByteParser,
};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{process::Command, sync::Mutex, time::sleep};
use tracing::{debug, error, info};

const VMPL: u32 = 1;
const CERT_FINGERPRINT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Reports {
    pub attestation: Arc<attestation_report::v2::AttestationReport>,
    pub raw_attestation: Vec<u8>,
    pub gpu_token: Option<String>,
}

pub struct HardwareReporter {
    reports: Arc<Mutex<Reports>>,
}

impl HardwareReporter {
    pub async fn new(gpu: GpuReportConfig, cert_fetcher: CertFetcher) -> anyhow::Result<Self> {
        let fingerprint = cert_fetcher.fetch_fingerprint().await.context("Failed to fetch cert fingerpring")?;
        let hardware_report = Self::fetch_hardware_report(&fingerprint).context("Failed to fetch hardware report")?;
        let raw_attestation = hardware_report.to_bytes()?.into();
        let reports = Reports {
            attestation: Arc::new(hardware_report.into()),
            raw_attestation,
            gpu_token: Self::fetch_gpu_report(&fingerprint, &gpu).await.context("Failed to fetch GPU report")?,
        };
        let reports = Arc::new(Mutex::new(reports));
        Worker::spawn(gpu, cert_fetcher, fingerprint, reports.clone());
        Ok(Self { reports })
    }

    pub async fn reports(&self) -> Reports {
        let reports = self.reports.lock().await;
        (*reports).clone()
    }

    fn fetch_hardware_report(fingerprint: &[u8; 32]) -> anyhow::Result<AttestationReport> {
        let mut data: [u8; 64] = [0; 64];
        // Version, bump if changed
        data[0] = 0;
        // Copy over the cert fingerprint
        data[1..33].copy_from_slice(fingerprint);

        info!("Generating hardware report using nonce {}", hex::encode(data));
        let mut fw = Firmware::open().context("unable to open /dev/sev-guest")?;
        let raw_report = fw.get_report(None, Some(data), Some(VMPL)).context("unable to fetch attestation report")?;
        let report = AttestationReport::from_bytes(&raw_report)?;
        Ok(report)
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

#[derive(Clone)]
pub enum GpuReportConfig {
    Enabled { attester_path: PathBuf },
    Disabled,
}

struct Worker {
    gpu: GpuReportConfig,
    cert_fetcher: CertFetcher,
    fingerprint: [u8; 32],
    reports: Arc<Mutex<Reports>>,
}

impl Worker {
    fn spawn(gpu: GpuReportConfig, cert_fetcher: CertFetcher, fingerprint: [u8; 32], reports: Arc<Mutex<Reports>>) {
        let worker = Self { gpu, cert_fetcher, fingerprint, reports };
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
        let hardware_report =
            HardwareReporter::fetch_hardware_report(&fingerprint).context("Failed to fetch hardware report")?;
        let raw_attestation = hardware_report.to_bytes()?.into();
        let gpu_token =
            HardwareReporter::fetch_gpu_report(&fingerprint, &self.gpu).await.context("Failed to fetch GPU report")?;
        self.fingerprint = fingerprint;
        *self.reports.lock().await =
            Reports { attestation: Arc::new(hardware_report.into()), raw_attestation, gpu_token };
        Ok(())
    }
}
