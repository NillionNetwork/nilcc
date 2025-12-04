use crate::verify::Processor;
use async_trait::async_trait;
use reqwest::get;
use sev::{
    certs::snp::{Certificate, ca::Chain},
    firmware::guest::AttestationReport,
};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
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
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> Result<Certs, FetcherError>;
}

/// A default implementation of the certificate fetcher.
pub struct DefaultCertificateFetcher {
    cache_path: PathBuf,
}

impl DefaultCertificateFetcher {
    pub fn new(cache_path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&cache_path)?;
        Ok(Self { cache_path })
    }

    async fn fetch_vcek(&self, processor: &Processor, report: &AttestationReport) -> Result<Certificate, FetcherError> {
        let identifier = ProcessorVcekIdentifier::new(processor.clone(), report)?;
        let cache_file_name = self.cache_path.join(identifier.cache_file_name());
        match self.load_cache_file(&cache_file_name)? {
            Some(cert) => match Certificate::from_bytes(&cert) {
                Ok(cert) => {
                    info!("Using cached VCEK certificate {}", cache_file_name.display());
                    return Ok(cert);
                }
                Err(e) => {
                    error!("Downloading VCEK because cached file {} is corrupted: {e}", cache_file_name.display());
                }
            },
            None => {
                info!("VCEK not found, downloading it");
            }
        };

        let url = identifier.kds_url();
        info!("Fetching VCEK from {url}");

        let response = get(url).await.and_then(|r| r.error_for_status()).map_err(FetcherError::FetchingVcek)?;
        let bytes = response.bytes().await.map_err(FetcherError::FetchingVcek)?.to_vec();
        let cert = Certificate::from_bytes(&bytes).map_err(FetcherError::ParsingVcek)?;
        self.cache_file(&cache_file_name, &bytes)?;
        Ok(cert)
    }

    async fn fetch_cert_chain(&self, processor: &Processor) -> Result<Chain, FetcherError> {
        let cache_file_name = self.cache_path.join(format!("{processor:?}.cert"));
        match self.load_cache_file(&cache_file_name)? {
            Some(chain) => match Chain::from_pem_bytes(&chain) {
                Ok(chain) => {
                    info!("Using cached certificate chain file {}", cache_file_name.display());
                    return Ok(chain);
                }
                Err(e) => {
                    error!(
                        "Downloading cert chain for processor {processor:?} because cached file {} is corrupted: {e}",
                        cache_file_name.display()
                    );
                }
            },
            None => {
                info!("Cert chain file for processor {processor:?} not found, downloading it");
            }
        };

        let url = format!("{KDS_CERT_SITE}/vcek/v1/{}/cert_chain", processor.to_kds_url());
        info!("Fetching CA chain from {url}");

        let response = get(url).await.and_then(|r| r.error_for_status()).map_err(FetcherError::FetchingCertChain)?;
        let bytes = response.bytes().await.map_err(FetcherError::FetchingCertChain)?.to_vec();
        let certificates = Chain::from_pem_bytes(&bytes).map_err(FetcherError::ParsingCertChain)?;
        self.cache_file(&cache_file_name, &bytes)?;
        Ok(certificates)
    }

    fn load_cache_file(&self, path: &Path) -> Result<Option<Vec<u8>>, FetcherError> {
        match File::open(path) {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).map_err(FetcherError::ReadCachedCert)?;
                Ok(Some(buffer))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(FetcherError::ReadCachedCert(e)),
        }
    }

    fn cache_file(&self, path: &Path, contents: &[u8]) -> Result<(), FetcherError> {
        fs::write(path, contents).map_err(FetcherError::WriteCachedCert)?;
        Ok(())
    }
}

#[async_trait]
impl CertificateFetcher for DefaultCertificateFetcher {
    async fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> Result<Certs, FetcherError> {
        let chain = self.fetch_cert_chain(processor).await?;
        let vcek = self.fetch_vcek(processor, report).await?;
        Ok(Certs { chain, vcek })
    }
}

struct ProcessorVcekIdentifier {
    processor: Processor,
    fmc: Option<u8>,
    bootloader: u8,
    tee: u8,
    snp: u8,
    microcode: u8,
    hw_id: String,
}

impl ProcessorVcekIdentifier {
    fn new(processor: Processor, report: &AttestationReport) -> Result<Self, FetcherError> {
        let tcb = report.reported_tcb;
        if let Processor::Turin = processor
            && tcb.fmc.is_none()
        {
            return Err(FetcherError::TurinFmc);
        }
        if report.chip_id.as_slice() == [0; 64] {
            return Err(FetcherError::ZeroHardwareId);
        }
        let hw_id = match processor {
            Processor::Turin => {
                let shorter_bytes: &[u8] = &report.chip_id[0..8];
                hex::encode(shorter_bytes)
            }
            _ => hex::encode(report.chip_id),
        };
        Ok(Self {
            processor,
            fmc: tcb.fmc,
            bootloader: tcb.bootloader,
            tee: tcb.tee,
            snp: tcb.snp,
            microcode: tcb.microcode,
            hw_id,
        })
    }

    fn kds_url(&self) -> String {
        let Self { processor, fmc, bootloader, tee, snp, microcode, hw_id } = self;
        let fmc_param = match fmc {
            Some(fmc) => format!("&fmcSPL={fmc:02}"),
            None => "".into(),
        };
        let processor = processor.to_kds_url();
        format!(
            "{KDS_CERT_SITE}/vcek/v1/{processor}/{hw_id}?blSPL={bootloader:02}&teeSPL={tee:02}&snpSPL={snp:02}&ucodeSPL={microcode:02}{fmc_param}"
        )
    }

    fn cache_file_name(&self) -> String {
        let Self { processor, fmc, bootloader, tee, snp, microcode, hw_id } = self;
        format!("{processor:?}-{fmc:02?}-{bootloader:02}-{tee:02}-{snp:02}-{microcode:02}-{hw_id}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetcherError {
    #[error("turin processors must have a fmc value")]
    TurinFmc,

    #[error("hardware ID is 0s on attestation report")]
    ZeroHardwareId,

    #[error("reading cached cert: {0}")]
    ReadCachedCert(io::Error),

    #[error("writing cached cert: {0}")]
    WriteCachedCert(io::Error),

    #[error("fetching AMD VCEK certificate: {0}")]
    FetchingVcek(reqwest::Error),

    #[error("fetching ACM cert chain: {0}")]
    FetchingCertChain(reqwest::Error),

    #[error("parsing AMD VCEK certificate: {0}")]
    ParsingVcek(io::Error),

    #[error("parsing AMD cert chain: {0}")]
    ParsingCertChain(io::Error),
}
