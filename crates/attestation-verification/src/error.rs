use crate::{
    certs::FetcherError, measurement::MeasurementHashError, report::ReportBundleError, verify::VerificationError,
};
use nilcc_artifacts::downloader::DownloadError;
use serde::Serialize;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ValidateError {
    #[error("invalid hex docker compose hash")]
    DockerComposeHash,

    #[error("creating cert cache directories: {0}")]
    CertCacheDirectories(io::Error),

    #[error("fetching report bundle: {0}")]
    ReportBundle(#[from] ReportBundleError),

    #[error(transparent)]
    MeasurementHash(#[from] MeasurementHashError),

    #[error("verifying report: {0}")]
    VerifyReports(#[from] VerificationError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidDockerComposeHash,
    InvalidTlsFingerprint,
    InvalidArtifacts,
    InvalidReport,
    InvalidAmdCerts,
    Filesystem,
    Request,
    Internal,
}

impl From<ValidateError> for ErrorCode {
    fn from(e: ValidateError) -> Self {
        use ErrorCode::*;
        match e {
            ValidateError::DockerComposeHash => InvalidDockerComposeHash,
            ValidateError::CertCacheDirectories(_) => Filesystem,
            ValidateError::ReportBundle(e) => match e {
                ReportBundleError::TlsFingerprint { .. } => InvalidTlsFingerprint,
                ReportBundleError::HttpClient(_) => Internal,
                ReportBundleError::FetchAttestation(_)
                | ReportBundleError::NoTlsInfo
                | ReportBundleError::TlsCertificate(_)
                | ReportBundleError::NotHttpsScheme
                | ReportBundleError::InvalidUrl(_)
                | ReportBundleError::MalformedPayload(_) => Request,
                ReportBundleError::DownloadArtifacts(e) => match e {
                    DownloadError::NoParent => Internal,
                    DownloadError::TargetDirectory(_) | DownloadError::TargetFile(_) => Filesystem,
                    DownloadError::DecodeMetadata(_) => InvalidArtifacts,
                    DownloadError::Download(_) => Request,
                },
            },
            ValidateError::MeasurementHash(_) => Internal,
            ValidateError::VerifyReports(e) => match e {
                VerificationError::FetchCerts(e) => match e {
                    FetcherError::TurinFmc | FetcherError::ZeroHardwareId => InvalidReport,
                    FetcherError::ReadCachedCert(_) | FetcherError::WriteCachedCert(_) => Filesystem,
                    FetcherError::FetchingVcek(_) | FetcherError::FetchingCertChain(_) => Request,
                    FetcherError::ParsingVcek(_) | FetcherError::ParsingCertChain(_) => InvalidAmdCerts,
                },
                VerificationError::CertVerification(_)
                | VerificationError::MalformedCertificate(_)
                | VerificationError::InvalidCertificate(_) => InvalidAmdCerts,
                VerificationError::DetectProcessor(_)
                | VerificationError::InvalidMeasurement { .. }
                | VerificationError::InvalidVcekPubKey
                | VerificationError::MalformedReportSignature
                | VerificationError::InvalidSignature
                | VerificationError::DebugAllowed => InvalidReport,
                VerificationError::SerializeReport(_) => Internal,
            },
        }
    }
}
