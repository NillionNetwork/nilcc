use crate::certs::{CertificateFetcher, Certs};
use anyhow::{anyhow, bail, Context};
use clap::ValueEnum;
use openssl::{ecdsa::EcdsaSig, sha::Sha384};
use serde::Deserialize;
use sev::{
    certs::snp::{Certificate, Verifiable},
    firmware::{guest::AttestationReport, host::CertType},
};
use std::io;
use tracing::info;
use x509_parser::{
    asn1_rs::Oid,
    der_parser::oid,
    prelude::{FromDer, X509Certificate, X509Extension},
    x509::X509Name,
};

pub struct ReportVerifier {
    processor: Option<Processor>,
    fetcher: Box<dyn CertificateFetcher>,
}

impl ReportVerifier {
    pub fn new(fetcher: Box<dyn CertificateFetcher>) -> Self {
        Self { processor: None, fetcher }
    }

    pub fn with_processor(mut self, processor: Processor) -> Self {
        self.processor = Some(processor);
        self
    }

    pub async fn verify_report(&self, report: AttestationReport) -> anyhow::Result<()> {
        let processor = match self.processor.clone() {
            Some(processor) => processor,
            None => {
                info!("Detecting processor type based on attestation report");
                Processor::try_from(&report).context("detecting processor")?
            }
        };
        info!("Using processor model {processor:?} for verification");

        let certs = self.fetcher.fetch_certs(&processor, &report).await.context("fetching certs")?;
        Self::verify_certs(&certs).context("verifying certs")?;

        Self::verify_report_signature(&certs.vcek, &report).context("verifying report signature")?;
        Self::verify_attestation_tcb(&certs.vcek, &report, &processor).context("verifying attestation TCB")?;
        Ok(())
    }

    fn verify_certs(certs: &Certs) -> Result<(), CertificateValidationError> {
        let ark = &certs.chain.ark;
        let ask = &certs.chain.ask;

        // Ensure ARK is self signed.
        match (ark, ark).verify() {
            Ok(()) => {}
            Err(e) => match e.kind() {
                io::ErrorKind::Other => return Err(CertificateValidationError::ArkNotSelfSigned),
                _ => return Err(CertificateValidationError::VerificationFailure("ARK", e.to_string())),
            },
        }

        // Ensure ARK signs ASK.
        match (ark, ask).verify() {
            Ok(()) => {}
            Err(e) => match e.kind() {
                io::ErrorKind::Other => return Err(CertificateValidationError::AskNotSignedByArk),
                _ => return Err(CertificateValidationError::VerificationFailure("ASK", e.to_string())),
            },
        }

        // Ensure ASK signs VCEK.
        match (ask, &certs.vcek).verify() {
            Ok(()) => {}
            Err(e) => match e.kind() {
                io::ErrorKind::Other => return Err(CertificateValidationError::VcekNotSignedByAsk),
                _ => return Err(CertificateValidationError::VerificationFailure("VCEK", e.to_string())),
            },
        }
        Ok(())
    }

    fn verify_report_signature(vcek: &Certificate, report: &AttestationReport) -> anyhow::Result<()> {
        let vek_pubkey = vcek
            .public_key()
            .context("getting VEK public key")?
            .ec_key()
            .context("converting VEK public key into ECkey")?;

        // Get the attestation report signature
        let ar_signature =
            EcdsaSig::try_from(&report.signature).context("Failed to get ECDSA signature from attestation report.")?;
        let mut report_bytes = Vec::new();
        report.write_bytes(&mut report_bytes)?;
        let signed_bytes = &report_bytes[0x0..0x2A0];

        let mut hasher = Sha384::new();
        hasher.update(signed_bytes);
        let digest = hasher.finish();

        // Verify signature
        if ar_signature
            .verify(digest.as_ref(), vek_pubkey.as_ref())
            .context("verifying attestation report signature")?
        {
            Ok(())
        } else {
            bail!("VEK did not sign the attestation report!")
        }
    }

    fn check_cert_bytes(ext: &X509Extension, val: &[u8]) -> anyhow::Result<bool> {
        let output = match ext.value[0] {
            // Integer
            0x2 => {
                if ext.value[1] != 0x1 && ext.value[1] != 0x2 {
                    bail!("invalid octet length encountered");
                } else if let Some(byte_value) = ext.value.last() {
                    byte_value == &val[0]
                } else {
                    false
                }
            }
            // Octet String
            0x4 => {
                if ext.value[1] != 0x40 {
                    bail!("invalid octet length encountered!");
                } else if ext.value[2..].len() != 0x40 {
                    bail!("invalid size of bytes encountered!");
                } else if val.len() != 0x40 {
                    bail!("invalid certificate harward id length encountered!")
                }

                &ext.value[2..] == val
            }
            // Legacy and others.
            _ => {
                // Keep around for a bit for old VCEK without x509 DER encoding.
                if ext.value.len() == 0x40 && val.len() == 0x40 {
                    ext.value == val
                } else {
                    bail!("invalid type encountered!");
                }
            }
        };
        Ok(output)
    }

    fn parse_common_name(field: &X509Name) -> anyhow::Result<CertType> {
        if let Some(val) = field.iter_common_name().next().and_then(|cn| cn.as_str().ok()) {
            match val.to_lowercase() {
                x if x.contains("ark") => Ok(CertType::ARK),
                x if x.contains("ask") | x.contains("sev") => Ok(CertType::ASK),
                x if x.contains("vcek") => Ok(CertType::VCEK),
                x if x.contains("vlek") => Ok(CertType::VLEK),
                x if x.contains("crl") => Ok(CertType::CRL),
                _ => Err(anyhow::anyhow!("Unknown certificate type encountered!")),
            }
        } else {
            bail!("certificate subject Common Name is unknown!")
        }
    }

    fn verify_attestation_tcb(
        vcek: &Certificate,
        report: &AttestationReport,
        processor: &Processor,
    ) -> anyhow::Result<()> {
        let vek_der = vcek.to_der().context("converting VEK to DER")?;
        let (_, vek_x509) = X509Certificate::from_der(&vek_der).context("creating x509 cert from DER")?;

        // Collect extensions from VEK
        let extensions = vek_x509.extensions_map().context("getting VEK Oids")?;

        let common_name: CertType = Self::parse_common_name(vek_x509.subject())?;

        // Compare bootloaders
        if let Some(cert_bl) = extensions.get(&SnpOid::BootLoader.oid()) {
            if !Self::check_cert_bytes(cert_bl, &report.reported_tcb.bootloader.to_le_bytes())? {
                bail!("report TCB boot loader and certificate boot loader mismatch encountered");
            }
        }

        // Compare TEE information
        if let Some(cert_tee) = extensions.get(&SnpOid::Tee.oid()) {
            if !Self::check_cert_bytes(cert_tee, &report.reported_tcb.tee.to_le_bytes())? {
                bail!("report TCB TEE and certificate TEE mismatch encountered");
            }
        }

        // Compare SNP information
        if let Some(cert_snp) = extensions.get(&SnpOid::Snp.oid()) {
            if !Self::check_cert_bytes(cert_snp, &report.reported_tcb.snp.to_le_bytes())? {
                bail!("report TCB SNP and Certificate SNP mismatch encountered");
            }
        }

        // Compare Microcode information
        if let Some(cert_ucode) = extensions.get(&SnpOid::Ucode.oid()) {
            if !Self::check_cert_bytes(cert_ucode, &report.reported_tcb.microcode.to_le_bytes())? {
                bail!("report TCB microcode and certificate microcode mismatch encountered");
            }
        }

        // Compare HWID information only on VCEK
        if common_name == CertType::VCEK {
            if let Some(cert_hwid) = extensions.get(&SnpOid::HwId.oid()) {
                if !Self::check_cert_bytes(cert_hwid, &*report.chip_id)? {
                    bail!("report TCB ID and certificate ID mismatch encountered");
                }
            }
        }

        if processor == &Processor::Turin {
            if report.version < 3 {
                bail!("Turin attestation is not supported in version 2 of the report");
            }
            if let Some(cert_fmc) = extensions.get(&SnpOid::Fmc.oid()) {
                if let Some(fmc) = report.reported_tcb.fmc {
                    if !Self::check_cert_bytes(cert_fmc, fmc.to_le_bytes().as_slice())? {
                        bail!("report TCB FMC and certificate FMC mismatch encountered");
                    }
                } else {
                    bail!("attestation report TCB FMC is not present in the report, but is expected for {processor:?} model");
                };
            }
        }
        Ok(())
    }
}

#[derive(ValueEnum, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Processor {
    /// 3rd Gen AMD EPYC Processor (Standard)
    Milan,

    /// 4th Gen AMD EPYC Processor (Standard)
    Genoa,

    /// 4th Gen AMD EPYC Processor (Performance)
    Bergamo,

    /// 4th Gen AMD EPYC Processor (Edge)
    Siena,

    /// 5th Gen AMD EPYC Processor (Standard)
    Turin,
}

impl Processor {
    pub(crate) fn to_kds_url(&self) -> &'static str {
        match self {
            Processor::Genoa | Processor::Siena | Processor::Bergamo => "Genoa",
            Processor::Milan => "Milan",
            Processor::Turin => "Turin",
        }
    }
}

impl TryFrom<&AttestationReport> for Processor {
    type Error = anyhow::Error;

    fn try_from(report: &AttestationReport) -> Result<Self, Self::Error> {
        if report.version < 3 {
            if [0u8; 64] == *report.chip_id {
                bail!("attestation report version is lower than 3 and Chip ID is all 0s");
            } else {
                let chip_id = *report.chip_id;
                if chip_id[8..64] == [0; 56] {
                    return Ok(Processor::Turin);
                } else {
                    bail!("attestation report could be either Milan or Genoa");
                }
            }
        }

        let family =
            report.cpuid_fam_id.ok_or_else(|| anyhow!("attestation report version 3+ is missing CPU family ID"))?;
        let model =
            report.cpuid_mod_id.ok_or_else(|| anyhow!("attestation report version 3+ is missing CPU model ID"))?;

        match family {
            0x19 => match model {
                0x0..=0xF => Ok(Processor::Milan),
                0x10..=0x1F | 0xA0..0xAF => Ok(Processor::Genoa),
                _ => Err(anyhow!("processor model not supported")),
            },
            0x1A => match model {
                0x0..=0x11 => Ok(Processor::Turin),
                _ => Err(anyhow!("processor model not supported")),
            },
            _ => Err(anyhow!("processor family not supported")),
        }
    }
}

enum SnpOid {
    BootLoader,
    Tee,
    Snp,
    Ucode,
    HwId,
    Fmc,
}

impl SnpOid {
    fn oid(&self) -> Oid {
        match self {
            SnpOid::BootLoader => oid!(1.3.6 .1 .4 .1 .3704 .1 .3 .1),
            SnpOid::Tee => oid!(1.3.6 .1 .4 .1 .3704 .1 .3 .2),
            SnpOid::Snp => oid!(1.3.6 .1 .4 .1 .3704 .1 .3 .3),
            SnpOid::Ucode => oid!(1.3.6 .1 .4 .1 .3704 .1 .3 .8),
            SnpOid::HwId => oid!(1.3.6 .1 .4 .1 .3704 .1 .4),
            SnpOid::Fmc => oid!(1.3.6 .1 .4 .1 .3704 .1 .3 .9),
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
enum CertificateValidationError {
    #[error("ARK is not self signed")]
    ArkNotSelfSigned,

    #[error("ASK is not signed by ARK")]
    AskNotSignedByArk,

    #[error("VCEK is not signed by ASK")]
    VcekNotSignedByAsk,

    #[error("{0} verification failure: {1}")]
    VerificationFailure(&'static str, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        ec::{EcGroup, EcKey},
        hash::MessageDigest,
        nid::Nid,
        pkey::{PKey, Private},
        x509::X509,
    };
    use rstest::rstest;
    use sev::certs::snp::ca::Chain;

    struct CertBuilder<'a> {
        signer: &'a PKey<Private>,
        owner: &'a PKey<Private>,
    }

    impl CertBuilder<'_> {
        fn make_cert(&self) -> Certificate {
            let mut builder = X509::builder().expect("failed to create builder");
            builder.set_pubkey(self.owner).expect("failed to set pubkey");
            builder.sign(self.signer, MessageDigest::sha256()).expect("failed to sign");
            Certificate::from(builder.build())
        }
    }

    struct Keys {
        ark: PKey<Private>,
        ask: PKey<Private>,
        vcek: PKey<Private>,
    }

    struct CertsBuilder;

    impl CertsBuilder {
        fn new_valid() -> Certs {
            let keys = Self::make_keys();
            // ark is self signed
            let ark = CertBuilder { signer: &keys.ark, owner: &keys.ark }.make_cert();

            // ask is signed by ark key
            let ask = CertBuilder { signer: &keys.ark, owner: &keys.ask }.make_cert();

            // vcek is signed by ask key
            let vcek = CertBuilder { signer: &keys.ask, owner: &keys.vcek }.make_cert();
            Certs { chain: Chain { ark, ask }, vcek }
        }

        fn new_ark_not_self_signed() -> Certs {
            let other_key = Self::make_key();
            let keys = Self::make_keys();
            let ark = CertBuilder { signer: &other_key, owner: &keys.ark }.make_cert();
            let ask = CertBuilder { signer: &keys.ark, owner: &keys.ask }.make_cert();
            let vcek = CertBuilder { signer: &keys.ask, owner: &keys.vcek }.make_cert();
            Certs { chain: Chain { ark, ask }, vcek }
        }

        fn new_ask_not_signed_by_ark() -> Certs {
            let other_key = Self::make_key();
            let keys = Self::make_keys();
            let ark = CertBuilder { signer: &keys.ark, owner: &keys.ark }.make_cert();
            let ask = CertBuilder { signer: &other_key, owner: &keys.ask }.make_cert();
            let vcek = CertBuilder { signer: &keys.ask, owner: &keys.vcek }.make_cert();
            Certs { chain: Chain { ark, ask }, vcek }
        }

        fn new_vcek_not_signed_by_ask() -> Certs {
            let other_key = Self::make_key();
            let keys = Self::make_keys();
            let ark = CertBuilder { signer: &keys.ark, owner: &keys.ark }.make_cert();
            let ask = CertBuilder { signer: &keys.ark, owner: &keys.ask }.make_cert();
            let vcek = CertBuilder { signer: &other_key, owner: &keys.vcek }.make_cert();
            Certs { chain: Chain { ark, ask }, vcek }
        }

        fn make_keys() -> Keys {
            let ark = Self::make_key();
            let ask = Self::make_key();
            let vcek = Self::make_key();
            Keys { ark, ask, vcek }
        }

        fn make_key() -> PKey<Private> {
            // NIST P-256 curve
            let nid = Nid::X9_62_PRIME256V1;
            let group = EcGroup::from_curve_name(nid).expect("invalid curve name");
            EcKey::generate(&group).expect("failed to generate key").try_into().expect("failed to convert key")
        }
    }

    #[test]
    fn validate_certificates() {
        let certs = CertsBuilder::new_valid();
        ReportVerifier::verify_certs(&certs).expect("verification failed");
    }

    #[rstest]
    #[case(CertsBuilder::new_ark_not_self_signed(), CertificateValidationError::ArkNotSelfSigned)]
    #[case(CertsBuilder::new_ask_not_signed_by_ark(), CertificateValidationError::AskNotSignedByArk)]
    #[case(CertsBuilder::new_vcek_not_signed_by_ask(), CertificateValidationError::VcekNotSignedByAsk)]
    fn invalid_cert_chain(#[case] certs: Certs, #[case] expected_error: CertificateValidationError) {
        let err = ReportVerifier::verify_certs(&certs).expect_err("verification succeeded");
        assert_eq!(err, expected_error);
    }

    #[test]
    fn valid_signature_verification() {
        let vcek = CertsBuilder::make_key();
        let vcek_cert = CertBuilder { signer: &vcek, owner: &vcek }.make_cert();
        let vcek = EcKey::try_from(vcek).expect("failed to construct EC key");
        let mut report = AttestationReport::default();

        let mut report_bytes = Vec::new();
        report.write_bytes(&mut report_bytes).unwrap();
        let signed_bytes = &report_bytes[0x0..0x2A0];

        let mut hasher = Sha384::new();
        hasher.update(signed_bytes);
        let digest = hasher.finish();

        let signature = EcdsaSig::sign(&digest, &vcek).expect("failed to sign");
        report.signature = signature.try_into().expect("could not convert signature");
        ReportVerifier::verify_report_signature(&vcek_cert, &report).expect("signature verification failed");
    }

    #[test]
    fn invalid_signature_verification() {
        let vcek = CertsBuilder::new_valid().vcek;
        let report = AttestationReport::default();
        ReportVerifier::verify_report_signature(&vcek, &report).expect_err("signature verification succeeded");
    }
}
