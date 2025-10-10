use crate::VmType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use std::borrow::Cow;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactsMetadata {
    /// Metadata about the build.
    // Note: this can be marked as required after everything artifacts < 0.2.0 are no longer used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildMetadata>,

    /// Information about the OVMF.
    pub ovmf: Artifact,

    /// Information about the initrd file.
    pub initrd: Artifact,

    /// Information about the CVM images.
    pub cvm: Cvm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildMetadata {
    /// The timestamp when the artifacts were built.
    #[serde(deserialize_with = "chrono::serde::ts_seconds::deserialize")]
    pub timestamp: DateTime<Utc>,

    /// The git hash the artifacts were built from.
    pub git_hash: String,

    /// The github action run ID that generated the artifacts.
    pub github_action_run_id: u64,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    /// The path to the file.
    pub path: String,

    /// The sha256 hash of the file file.
    #[serde_as(as = "Hex")]
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cvm {
    /// The kernel command line parameters.
    pub cmdline: KernelCommandLine,

    /// The CVM images.
    pub images: CvmImages,
}

pub struct KernelArgs<'a> {
    pub docker_compose_hash: &'a str,
    pub filesystem_root_hash: &'a [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelCommandLine(pub String);

impl KernelCommandLine {
    /// Render these command line arguments.
    pub fn render(&self, args: KernelArgs) -> Result<String, MissingCommandLineParameter> {
        let KernelArgs { docker_compose_hash, filesystem_root_hash } = args;
        let filesystem_root_hash = hex::encode(filesystem_root_hash);
        let pairs =
            &[("{VERITY_ROOT_HASH}", filesystem_root_hash.as_str()), ("{DOCKER_COMPOSE_HASH}", docker_compose_hash)];
        let mut output = Cow::Borrowed(&self.0);
        for (key, replacement) in pairs {
            if !output.contains(key) {
                return Err(MissingCommandLineParameter(key));
            }
            output = Cow::Owned(output.replace(key, replacement));
        }
        Ok(output.into_owned())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("missing kernel command line parameter: {0}")]
pub struct MissingCommandLineParameter(&'static str);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CvmImages {
    /// Information about the CPU CVM image.
    pub cpu: CvmImage,

    /// Information about the GPU CVM image.
    pub gpu: CvmImage,
}

impl CvmImages {
    pub fn resolve(&self, vm_type: VmType) -> &CvmImage {
        match vm_type {
            VmType::Cpu => &self.cpu,
            VmType::Gpu => &self.gpu,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CvmImage {
    /// Information about the CVM disk.
    pub disk: CvmDisk,

    /// Information about the verity disk.
    pub verity: Verity,

    /// Information about the kernel.
    pub kernel: Artifact,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CvmDisk {
    /// The artifact itself.
    #[serde(flatten)]
    pub artifact: Artifact,

    /// The disk format.
    pub format: DiskFormat,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verity {
    /// The verity disk.
    pub disk: VerityDisk,

    /// The verity root hash.
    #[serde_as(as = "Hex")]
    pub root_hash: [u8; 32],
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerityDisk {
    /// The path to the disk image.
    pub path: String,

    /// The disk format.
    pub format: DiskFormat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiskFormat {
    /// A disk in raw format.
    Raw,

    /// A qemu qcow2 disk.
    Qcow2,
}

impl fmt::Display for DiskFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Raw => "raw",
            Self::Qcow2 => "qcow2",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde() {
        let input = r#"{ "ovmf": { "path": "vm_images/ovmf/OVMF.fd", "sha256": "e842c3c58a54172592f6345ae7f44b1c7fe7c76af9578765932a77674e4475bb" }, "initrd": { "path": "initramfs/initramfs.cpio.gz", "sha256": "115c25046d4f357edae4506be45c5c6a79543a15db1b4ef2ec94bcfe70bd66ec" }, "cvm": { "cmdline": "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}", "images": { "cpu": { "disk": { "path": "vm_images/cvm-cpu.qcow2", "format": "qcow2", "sha256": "1287785f6a6a2cb08f0c25b864f8976a76e7e0d2a7c906738e2bc374266b3708" }, "verity": { "disk": { "path": "vm_images/cvm-cpu-verity/verity-hash-dev", "format": "raw" }, "root_hash": "4324eabc4d0d9aa2aed99f7ac16bd118473f455ab97841674afd3bc318d755cb" }, "kernel": { "path": "vm_images/kernel/cpu-vmlinuz", "sha256": "92cefd4d94338ad808d3c1a1be3b1d166d92a654fcb52aecdbe8905e7970817e" } }, "gpu": { "disk": { "path": "vm_images/cvm-gpu.qcow2", "format": "qcow2", "sha256": "1287785f6a6a2cb08f0c25b864f8976a76e7e0d2a7c906738e2bc374266b3708" }, "verity": { "disk": { "path": "vm_images/cvm-gpu-verity/verity-hash-dev", "format": "raw" }, "root_hash": "4324eabc4d0d9aa2aed99f7ac16bd118473f455ab97841674afd3bc318d755cb" }, "kernel": { "path": "vm_images/kernel/gpu-vmlinuz", "sha256": "92cefd4d94338ad808d3c1a1be3b1d166d92a654fcb52aecdbe8905e7970817e" } } } } }"#;
        let meta: ArtifactsMetadata = serde_json::from_str(input).expect("failed to deserialize");
        let serialized = serde_json::to_string(&meta).expect("failed to serialize");
        assert_eq!(serde_json::from_str::<ArtifactsMetadata>(&serialized).expect("failed ot parse"), meta);
    }

    #[test]
    fn render_valid_kernel_command_line() {
        let cmdline = "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}";
        let cmdline = KernelCommandLine(cmdline.into());
        let rendered = cmdline
            .render(KernelArgs { docker_compose_hash: "aaa", filesystem_root_hash: &[0; 32] })
            .expect("failed to render");
        let expected = "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash=0000000000000000000000000000000000000000000000000000000000000000 state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash=aaa";
        assert_eq!(rendered, expected);
    }
}
