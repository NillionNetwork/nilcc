use anyhow::{bail, Context};
use sev::firmware::guest::{AttestationReport, Firmware};
use std::{path::PathBuf, process::Stdio};
use tokio::process::Command;

pub const VMPL: u32 = 1;

pub(crate) type ReportData = [u8; 64];

pub struct HardwareReporter {
    gpu_attester_path: PathBuf,
}

impl HardwareReporter {
    pub fn new(gpu_attester_path: PathBuf) -> Self {
        Self { gpu_attester_path }
    }

    pub async fn gpu_report(&self, nonce: &str) -> anyhow::Result<String> {
        let output = Command::new(&self.gpu_attester_path)
            .arg(nonce)
            .stderr(Stdio::piped())
            .output()
            .await
            .context("failed to invoke GPU attester")?;
        if output.status.success() {
            String::from_utf8(output.stdout).context("invalid utf8 token")
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("could not generate GPU report: {stderr}")
        }
    }

    pub fn hardware_report(&self, data: ReportData) -> anyhow::Result<AttestationReport> {
        let mut fw = Firmware::open().context("unable to open /dev/sev-guest")?;
        let raw_report = fw.get_report(None, Some(data), Some(VMPL)).context("unable to fetch attestation report")?;
        Ok(AttestationReport::from_bytes(&raw_report)?)
    }
}
