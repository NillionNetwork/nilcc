use crate::verify::Processor;
use anyhow::{bail, Context};
use reqwest::{blocking::get, StatusCode};
use sev::{
    certs::snp::{ca::Chain, Certificate},
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
pub trait CertificateFetcher: Send + Sync + 'static {
    /// Fetch certificates.
    fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs>;
}

/// A default implementation of the certificate fetcher.
pub struct DefaultCertificateFetcher {
    cache_path: PathBuf,
}

impl DefaultCertificateFetcher {
    pub fn new(cache_path: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&cache_path).context("creating cache directory")?;
        Ok(Self { cache_path })
    }

    fn fetch_vcek(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certificate> {
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

        let response = get(url).context("unable to send request for VCEK")?;
        match response.status() {
            StatusCode::OK => {
                let bytes = response.bytes().context("unable to parse VCEK")?.to_vec();
                let cert = Certificate::from_bytes(&bytes).context("parsing VCEK")?;
                self.cache_file(&cache_file_name, &bytes)?;
                Ok(cert)
            }
            status => bail!("unable to fetch VCEK from URL: {status:?}"),
        }
    }

    fn fetch_cert_chain(&self, processor: &Processor) -> anyhow::Result<Chain> {
        let cache_file_name = self.cache_path.join(format!("{processor:?}.cert"));
        match self.load_cache_file(&cache_file_name)? {
            Some(chain) => {
                match Chain::from_pem_bytes(&chain) {
                    Ok(chain) => {
                        info!("Using cached certificate chain file {}", cache_file_name.display());
                        return Ok(chain);
                    }
                    Err(e) => {
                        error!("Downloading cert chain for processor {processor:?} because cached file {} is corrupted: {e}", cache_file_name.display());
                    }
                }
            }
            None => {
                info!("Cert chain file for processor {processor:?} not found, downloading it");
            }
        };

        let url = format!("{KDS_CERT_SITE}/vcek/v1/{}/cert_chain", processor.to_kds_url());
        info!("Fetching CA chain from {url}");

        let rsp = get(url).context("unable to send request for certs to URL")?;
        match rsp.status() {
            StatusCode::OK => {
                // Parse the request
                let body = rsp.bytes().context("unable to parse AMD certificate chain")?.to_vec();
                let certificates = Chain::from_pem_bytes(&body)?;
                self.cache_file(&cache_file_name, &body)?;
                Ok(certificates)
            }
            status => bail!("unable to fetch certificate: {status:?}"),
        }
    }

    fn load_cache_file(&self, path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
        match File::open(path) {
            Ok(mut file) => {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer).context("reading cache file")?;
                Ok(Some(buffer))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("opening cache file"),
        }
    }

    fn cache_file(&self, path: &Path, contents: &[u8]) -> anyhow::Result<()> {
        fs::write(path, contents).context("writing to cache file")?;
        Ok(())
    }
}

impl CertificateFetcher for DefaultCertificateFetcher {
    fn fetch_certs(&self, processor: &Processor, report: &AttestationReport) -> anyhow::Result<Certs> {
        info!("Fetching certificates from AMD API");
        let chain = self.fetch_cert_chain(processor).context("fetching cert chain")?;
        let vcek = self.fetch_vcek(processor, report).context("fetching VCEK")?;
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
    fn new(processor: Processor, report: &AttestationReport) -> anyhow::Result<Self> {
        let tcb = report.reported_tcb;
        if let Processor::Turin = processor {
            if tcb.fmc.is_none() {
                bail!("Turin processors must have a fmc value");
            }
        }
        if report.chip_id.as_slice() == [0; 64] {
            bail!("hardware ID is 0s on attestation report");
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
