use crate::{report::VMPL, verify::Processor};
use anyhow::{bail, Context};
use async_trait::async_trait;
use reqwest::{get, StatusCode};
use sev::{
    certs::snp::{ca::Chain, Certificate},
    firmware::{
        guest::{AttestationReport, Firmware},
        host::{CertTableEntry, CertType},
    },
};
use tracing::{error, info};

const KDS_CERT_SITE: &str = "https://kdsintf.amd.com";

/// The set of certificates needed to validate a report.
pub struct Certs {
    /// The certificate chain, which includes the ARK and ASK.
    pub chain: Chain,

    /// The VCEK certificate.
    pub vcek: Certificate,
}

/// An interface to fetch certificates.
#[async_trait]
pub trait CertificateFetcher: Send + Sync + 'static {
    /// Fetch certificates.
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs>;
}

/// The policy used when fetching certificates
pub enum CertFetchPolicy {
    /// Prefer fetching the certificates from hypervisor memory.
    ///
    /// This will fall back to using the AMD KDS API if there's an error fetching them from memory.
    PreferHardware,

    /// Fetch the certificates from the AMD KMS API.
    AmdKmsApi,
}

/// A default implementation of the certificate fetcher.
pub struct DefaultCertificateFetcher {
    policy: CertFetchPolicy,
}

impl DefaultCertificateFetcher {
    /// Construct a new fetcher using the given policy.
    pub fn new(policy: CertFetchPolicy) -> Self {
        Self { policy }
    }

    async fn fetch_vcek(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certificate> {
        let hw_id = if report.chip_id.as_slice() != [0; 64] {
            match processor {
                Processor::Turin => {
                    let shorter_bytes: &[u8] = &report.chip_id[0..8];
                    hex::encode(shorter_bytes)
                }
                _ => hex::encode(report.chip_id),
            }
        } else {
            bail!("hardware ID is 0s on attestation report");
        };

        // Request VCEK from KDS
        let url: String = match processor {
            Processor::Turin => {
                let fmc = if let Some(fmc) = report.reported_tcb.fmc {
                    fmc
                } else {
                    bail!("Turin processors must have a fmc value");
                };
                format!(
                    "{KDS_CERT_SITE}/vcek/v1/{}/\
                    {hw_id}?fmcSPL={:02}&blSPL={:02}&teeSPL={:02}&snpSPL={:02}&ucodeSPL={:02}",
                    processor.to_kds_url(),
                    fmc,
                    report.reported_tcb.bootloader,
                    report.reported_tcb.tee,
                    report.reported_tcb.snp,
                    report.reported_tcb.microcode
                )
            }
            _ => {
                format!(
                    "{KDS_CERT_SITE}/vcek/v1/{}/\
                    {hw_id}?blSPL={:02}&teeSPL={:02}&snpSPL={:02}&ucodeSPL={:02}",
                    processor.to_kds_url(),
                    report.reported_tcb.bootloader,
                    report.reported_tcb.tee,
                    report.reported_tcb.snp,
                    report.reported_tcb.microcode
                )
            }
        };

        info!("Fetching VCEK from {url}");
        let response = get(url).await.context("unable to send request for VCEK")?;
        match response.status() {
            StatusCode::OK => {
                let bytes = response.bytes().await.context("unable to parse VCEK")?.to_vec();
                let cert = Certificate::from_bytes(&bytes).context("parsing VCEK")?;
                Ok(cert)
            }
            status => bail!("unable to fetch VCEK from URL: {status:?}"),
        }
    }

    async fn fetch_cert_chain(&self, processor: &Processor) -> anyhow::Result<Chain> {
        let url = format!("{KDS_CERT_SITE}/vcek/v1/{}/cert_chain", processor.to_kds_url());
        info!("Fetching CA chain from {url}");

        let rsp = get(url).await.context("unable to send request for certs to URL")?;
        match rsp.status() {
            StatusCode::OK => {
                // Parse the request
                let body = rsp.bytes().await.context("unable to parse AMD certificate chain")?.to_vec();
                let certificates = Chain::from_pem_bytes(&body)?;
                Ok(certificates)
            }
            status => bail!("unable to fetch certificate: {status:?}"),
        }
    }

    fn fetch_from_memory(&self) -> anyhow::Result<Certs> {
        let mut fw = Firmware::open().context("unable to open /dev/sev-guest")?;
        // Create random data and create an extended report to get the certificates from memory.
        let data = rand::random();
        let (_, certs) =
            fw.get_ext_report(None, Some(data), Some(VMPL)).context("unable to fetch attestation report")?;
        let Some(certs) = certs else {
            bail!("no certificates returned when fetching extended report");
        };
        let mut ark = None;
        let mut ask = None;
        let mut vcek = None;
        for cert in certs {
            let CertTableEntry { cert_type, data } = cert;
            let target_cert = match cert_type {
                CertType::ARK => &mut ark,
                CertType::ASK => &mut ask,
                CertType::VCEK => &mut vcek,
                _ => continue,
            };
            if target_cert.is_some() {
                bail!("found more than one {target_cert:?} certificate");
            }
            let parsed_cert = Certificate::from_bytes(&data).context(format!("parsing {cert_type:?}"))?;
            *target_cert = Some(parsed_cert);
        }
        let Some(ark) = ark else {
            bail!("ARK not found");
        };
        let Some(ask) = ask else {
            bail!("ASK not found");
        };
        let Some(vcek) = vcek else {
            bail!("VCEK not found");
        };
        Ok(Certs { chain: Chain { ark, ask }, vcek })
    }
}

#[async_trait]
impl CertificateFetcher for DefaultCertificateFetcher {
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs> {
        if let CertFetchPolicy::PreferHardware = &self.policy {
            info!("Fetching certificates from memory");
            match self.fetch_from_memory() {
                Ok(certs) => return Ok(certs),
                Err(e) => {
                    error!("Failed to fetch certificates from memory: {e}");
                }
            }
        }
        info!("Fetching certificates from AMD API");
        let chain = self.fetch_cert_chain(processor).await.context("fetching cert chain")?;
        let vcek = self.fetch_vcek(processor, report).await.context("fetching VCEK")?;
        Ok(Certs { chain, vcek })
    }
}
