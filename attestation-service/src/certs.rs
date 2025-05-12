use crate::verify::Processor;
use anyhow::{bail, Context};
use async_trait::async_trait;
use reqwest::{get, StatusCode};
use sev::{
    certs::snp::{ca::Chain, Certificate},
    firmware::guest::AttestationReport,
};
use tracing::info;

const KDS_CERT_SITE: &str = "https://kdsintf.amd.com";

/// The set of certificates needed to validate a report.
pub struct Certs {
    /// The certificate chain, which includes the ARK and ASK.
    pub chain: Chain,

    /// The VCEK certificate.
    pub vcek: Certificate,
}

#[async_trait]
pub trait CertificateFetcher: Send + Sync + 'static {
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs>;
}

/// A default implementation of the certificate fetcher.
pub struct DefaultCertificateFetcher;

impl DefaultCertificateFetcher {
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
}

#[async_trait]
impl CertificateFetcher for DefaultCertificateFetcher {
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs> {
        let chain = self.fetch_cert_chain(processor).await.context("fetching cert chain")?;
        let vcek = self.fetch_vcek(processor, report).await.context("fetching VCEK")?;
        Ok(Certs { chain, vcek })
    }
}
