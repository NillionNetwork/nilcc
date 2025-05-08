use anyhow::Context;
use sev::firmware::guest::{AttestationReport, Firmware};

const VMPL: u32 = 1;

pub(crate) type ReportData = [u8; 64];

pub fn request_hardware_report(data: ReportData) -> anyhow::Result<AttestationReport> {
    let mut fw = Firmware::open().context("unable to open /dev/sev-guest")?;
    let raw_report = fw.get_report(None, Some(data), Some(VMPL)).context("unable to fetch attestation report")?;
    Ok(AttestationReport::from_bytes(&raw_report)?)
}
