use crate::metadata::ArtifactsMetadata;
use std::{fmt, path::PathBuf};

pub mod downloader;
pub mod metadata;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VmType {
    Cpu,
    Gpu,
}

impl fmt::Display for VmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
        }
    }
}

// TODO: remove this and use metadata instead
#[derive(Clone, Debug)]
pub struct Artifacts {
    pub metadata: ArtifactsMetadata,
    pub ovmf_path: PathBuf,
    pub initrd_path: PathBuf,
}
