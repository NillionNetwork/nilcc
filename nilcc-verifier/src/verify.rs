use crate::certs::{CertificateFetcher, Certs, FetcherError};
use clap::ValueEnum;
use openssl::{ecdsa::EcdsaSig, sha::Sha384};
use serde::Deserialize;
use sev::{
    certs::snp::{Certificate, Verifiable},
    firmware::{guest::AttestationReport, host::CertType},
    parser::ByteParser,
};
use std::io;
use tracing::{info, warn};
use x509_parser::{
    asn1_rs::Oid,
    der_parser::oid,
    prelude::{FromDer, X509Certificate, X509Extension},
    x509::X509Name,
};

pub struct ReportVerifier {
    fetcher: Box<dyn CertificateFetcher>,
}

impl ReportVerifier {
    pub fn new(fetcher: Box<dyn CertificateFetcher>) -> Self {
        Self { fetcher }
    }

    pub fn verify_report(&self, report: AttestationReport, measurement: &[u8]) -> Result<(), VerificationError> {
        let processor = Self::detect_processor(&report)?;
        info!("Using processor model {processor:?} for verification");

        let certs = self.fetcher.fetch_certs(&processor, &report)?;
        Self::verify_certs(&certs)?;

        if report.measurement.as_slice() != measurement {
            return Err(VerificationError::InvalidMeasurement {
                expected: hex::encode(measurement),
                actual: hex::encode(report.measurement),
            });
        }
        info!("Measurement matches expected: {}", hex::encode(measurement));

        Self::verify_report_signature(&certs.vcek, &report)?;
        Self::verify_attestation_tcb(&certs.vcek, &report, &processor)?;
        info!("Verification successful");
        Ok(())
    }

    fn detect_processor(report: &AttestationReport) -> Result<Processor, VerificationError> {
        info!("Detecting processor type based on attestation report");
        match Processor::try_from(report) {
            Ok(processor) => Ok(processor),
            Err(FromReportError::AmbiguousMilanGenoa) => {
                warn!("Processor could be Milan or Genoa, assuming Genoa");
                Ok(Processor::Genoa)
            }
            Err(e) => Err(VerificationError::DetectProcessor(e)),
        }
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

    fn verify_report_signature(vcek: &Certificate, report: &AttestationReport) -> Result<(), VerificationError> {
        use VerificationError::*;
        let vek_pubkey = vcek.public_key().map_err(|_| InvalidVcekPubKey)?.ec_key().map_err(|_| InvalidVcekPubKey)?;

        let signature = EcdsaSig::try_from(&report.signature).map_err(|_| MalformedReportSignature)?;
        let report_bytes = report.to_bytes().map_err(SerializeReport)?;
        let signed_bytes = &report_bytes[0x0..0x2A0];

        let mut hasher = Sha384::new();
        hasher.update(signed_bytes);
        let digest = hasher.finish();

        // Verify signature
        if signature.verify(digest.as_ref(), vek_pubkey.as_ref()).map_err(|_| InvalidSignature)? {
            Ok(())
        } else {
            Err(InvalidSignature)
        }
    }

    fn check_cert_bytes(ext: &X509Extension, val: &[u8]) -> Result<bool, VerificationError> {
        use VerificationError::InvalidCertificate;
        let output = match ext.value[0] {
            // Integer
            0x2 => {
                if ext.value[1] != 0x1 && ext.value[1] != 0x2 {
                    return Err(InvalidCertificate("invalid octet length encountered"));
                } else if let Some(byte_value) = ext.value.last() {
                    byte_value == &val[0]
                } else {
                    false
                }
            }
            // Octet String
            0x4 => {
                if ext.value[1] != 0x40 {
                    return Err(InvalidCertificate("invalid octet length encountered!"));
                } else if ext.value[2..].len() != 0x40 {
                    return Err(InvalidCertificate("invalid size of bytes encountered!"));
                } else if val.len() != 0x40 {
                    return Err(InvalidCertificate("invalid certificate harward id length encountered!"));
                }

                &ext.value[2..] == val
            }
            // Legacy and others.
            _ => {
                // Keep around for a bit for old VCEK without x509 DER encoding.
                if ext.value.len() == 0x40 && val.len() == 0x40 {
                    ext.value == val
                } else {
                    return Err(InvalidCertificate("invalid type encountered!"));
                }
            }
        };
        Ok(output)
    }

    fn parse_common_name(field: &X509Name) -> Result<CertType, VerificationError> {
        if let Some(val) = field.iter_common_name().next().and_then(|cn| cn.as_str().ok()) {
            match val.to_lowercase() {
                x if x.contains("ark") => Ok(CertType::ARK),
                x if x.contains("ask") | x.contains("sev") => Ok(CertType::ASK),
                x if x.contains("vcek") => Ok(CertType::VCEK),
                x if x.contains("vlek") => Ok(CertType::VLEK),
                x if x.contains("crl") => Ok(CertType::CRL),
                _ => Err(VerificationError::InvalidCertificate("unknown certificate type encountered")),
            }
        } else {
            Err(VerificationError::InvalidCertificate("certificate subject Common Name is unknown"))
        }
    }

    fn verify_attestation_tcb(
        vcek: &Certificate,
        report: &AttestationReport,
        processor: &Processor,
    ) -> Result<(), VerificationError> {
        use VerificationError::*;
        let vek_der = vcek.to_der().map_err(|e| MalformedCertificate(e.to_string()))?;
        let (_, vek_x509) = X509Certificate::from_der(&vek_der).map_err(|e| MalformedCertificate(e.to_string()))?;

        // Collect extensions from VEK
        let extensions = vek_x509.extensions_map().map_err(|_| InvalidCertificate("no extensions map"))?;

        let common_name: CertType = Self::parse_common_name(vek_x509.subject())?;

        // Compare bootloaders
        if let Some(cert_bl) = extensions.get(&SnpOid::BootLoader.oid())
            && !Self::check_cert_bytes(cert_bl, &report.reported_tcb.bootloader.to_le_bytes())?
        {
            return Err(InvalidCertificate("report TCB boot loader and certificate boot loader mismatch encountered"));
        }

        // Compare TEE information
        if let Some(cert_tee) = extensions.get(&SnpOid::Tee.oid())
            && !Self::check_cert_bytes(cert_tee, &report.reported_tcb.tee.to_le_bytes())?
        {
            return Err(InvalidCertificate("report TCB TEE and certificate TEE mismatch encountered"));
        }

        // Compare SNP information
        if let Some(cert_snp) = extensions.get(&SnpOid::Snp.oid())
            && !Self::check_cert_bytes(cert_snp, &report.reported_tcb.snp.to_le_bytes())?
        {
            return Err(InvalidCertificate("report TCB SNP and Certificate SNP mismatch encountered"));
        }

        // Compare Microcode information
        if let Some(cert_ucode) = extensions.get(&SnpOid::Ucode.oid())
            && !Self::check_cert_bytes(cert_ucode, &report.reported_tcb.microcode.to_le_bytes())?
        {
            return Err(InvalidCertificate("report TCB microcode and certificate microcode mismatch encountered"));
        }

        // Compare HWID information only on VCEK
        if common_name == CertType::VCEK
            && let Some(cert_hwid) = extensions.get(&SnpOid::HwId.oid())
            && !Self::check_cert_bytes(cert_hwid, &report.chip_id)?
        {
            return Err(InvalidCertificate("report TCB ID and certificate ID mismatch encountered"));
        }

        if processor == &Processor::Turin {
            if report.version < 3 {
                return Err(InvalidCertificate("Turin attestation is not supported in version 2 of the report"));
            }
            if let Some(cert_fmc) = extensions.get(&SnpOid::Fmc.oid()) {
                if let Some(fmc) = report.reported_tcb.fmc {
                    if !Self::check_cert_bytes(cert_fmc, fmc.to_le_bytes().as_slice())? {
                        return Err(InvalidCertificate("report TCB FMC and certificate FMC mismatch encountered"));
                    }
                } else {
                    return Err(InvalidCertificate(
                        "attestation report TCB FMC is not present in the report, but is expected for {processor:?} model",
                    ));
                };
            }
        }
        Ok(())
    }
}

#[derive(ValueEnum, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Processor {
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
    type Error = FromReportError;

    fn try_from(report: &AttestationReport) -> Result<Self, Self::Error> {
        if report.version < 3 {
            if report.chip_id == [0; 64] {
                return Err(FromReportError::ZeroChipIp);
            } else {
                let chip_id = report.chip_id;
                if chip_id[8..64] == [0; 56] {
                    return Ok(Processor::Turin);
                } else {
                    return Err(FromReportError::AmbiguousMilanGenoa);
                }
            }
        }

        let family = report.cpuid_fam_id.ok_or(FromReportError::MissingFamilyId)?;
        let model = report.cpuid_mod_id.ok_or(FromReportError::MissingModelId)?;

        match family {
            0x19 => match model {
                0x0..=0xF => Ok(Processor::Milan),
                0x10..=0x1F | 0xA0..0xAF => Ok(Processor::Genoa),
                _ => Err(FromReportError::ModelNotSupported),
            },
            0x1A => match model {
                0x0..=0x11 => Ok(Processor::Turin),
                _ => Err(FromReportError::ModelNotSupported),
            },
            _ => Err(FromReportError::FamilyNotSupported),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("failed to fetch certificates: {0}")]
    FetchCerts(#[from] FetcherError),

    #[error("failed to verity certificates: {0}")]
    CertVerification(#[from] CertificateValidationError),

    #[error("failed to detect processor: {0}")]
    DetectProcessor(FromReportError),

    #[error("invalid measurement hash, expected = {expected}, got = {actual}")]
    InvalidMeasurement { expected: String, actual: String },

    #[error("invalid VCEK public key")]
    InvalidVcekPubKey,

    #[error("malformed report signature")]
    MalformedReportSignature,

    #[error("invalid report signature")]
    InvalidSignature,

    #[error("failed to serialize report: {0}")]
    SerializeReport(io::Error),

    #[error("malformed AMD certificate: {0}")]
    MalformedCertificate(String),

    #[error("invalid AMD certificate: {0}")]
    InvalidCertificate(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FromReportError {
    #[error("attestation report version is lower than 3 and Chip ID is all 0s")]
    ZeroChipIp,

    #[error("attestation report could be either Milan or Genoa")]
    AmbiguousMilanGenoa,

    #[error("report version 3+ is missing family ID")]
    MissingFamilyId,

    #[error("report version 3+ is missing model ID")]
    MissingModelId,

    #[error("processor model not supported")]
    ModelNotSupported,

    #[error("processor family not supported")]
    FamilyNotSupported,
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
    fn oid(&self) -> Oid<'_> {
        match self {
            SnpOid::BootLoader => oid!(1.3.6.1.4.1.3704.1.3.1),
            SnpOid::Tee => oid!(1.3.6.1.4.1.3704.1.3.2),
            SnpOid::Snp => oid!(1.3.6.1.4.1.3704.1.3.3),
            SnpOid::Ucode => oid!(1.3.6.1.4.1.3704.1.3.8),
            SnpOid::HwId => oid!(1.3.6.1.4.1.3704.1.4),
            SnpOid::Fmc => oid!(1.3.6.1.4.1.3704.1.3.9),
        }
    }
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum CertificateValidationError {
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
        let report_json = r#"{"version":3,"guest_svn":0,"policy":196608,"family_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"image_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"vmpl":1,"sig_algo":1,"current_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"plat_info":5,"key_info":0,"report_data":[0,60,221,26,64,207,190,76,225,105,36,230,108,147,53,33,109,126,113,156,220,216,185,215,175,255,37,200,229,104,105,56,190,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"measurement":[133,218,39,154,206,134,74,150,158,59,191,220,170,182,127,243,1,116,2,183,210,43,162,65,82,155,29,74,121,185,166,148,43,137,212,230,218,103,71,200,1,226,114,104,50,85,174,75],"host_data":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"id_key_digest":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"author_key_digest":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"report_id":[141,246,201,117,45,5,37,129,218,56,1,145,154,84,173,74,69,142,27,48,209,37,52,206,67,156,46,182,39,2,34,101],"report_id_ma":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],"reported_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"cpuid_fam_id":25,"cpuid_mod_id":17,"cpuid_step":1,"chip_id":[83,104,118,136,195,115,97,41,69,129,48,75,25,36,41,224,178,186,159,238,226,71,206,49,193,30,136,232,198,195,91,251,64,179,47,143,154,245,53,8,126,216,226,55,111,186,64,60,15,33,178,120,21,11,189,96,164,173,48,229,145,223,253,109],"committed_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"current":{"major":1,"minor":55,"build":39},"committed":{"major":1,"minor":55,"build":39},"launch_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"signature":{"r":[117,157,183,45,56,121,141,255,189,223,139,156,55,142,7,194,168,53,111,152,56,159,90,145,196,95,77,32,129,207,84,100,229,238,49,93,144,144,125,106,116,72,22,205,181,27,14,233,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"s":[153,219,194,30,7,26,10,68,95,157,246,221,70,91,217,116,69,100,124,253,96,120,235,241,153,215,136,80,25,142,0,203,189,184,52,117,209,56,125,230,187,38,187,244,100,230,18,97,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#;
        let mut report: AttestationReport =
            serde_json::from_str::<attestation_report::v1::AttestationReport>(&report_json)
                .expect("failed to parse")
                .into();

        let report_bytes = report.to_bytes().unwrap();
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
