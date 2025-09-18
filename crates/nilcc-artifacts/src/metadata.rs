use crate::VmType;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
use serde_with::serde_as;
use std::borrow::Cow;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactsMetadata {
    /// Information about the kernel build.
    pub kernel: PackageMetadata,

    /// Information about the qemu build.
    pub qemu: PackageMetadata,

    /// Information about the OVMF.
    pub ovmf: Artifact,

    /// Information about the initrd file.
    pub initrd: Artifact,

    /// Information about the CVM images.
    pub cvm: Cvm,
}

impl ArtifactsMetadata {
    pub fn legacy(meta: LegacyMetadata) -> Self {
        Self {
            kernel: PackageMetadata { commit: "".into() },
            qemu: PackageMetadata { commit: "".into() },
            ovmf: Artifact { path: "vm_images/ovmf/OVMF.fd".into(), sha256: [0; 32] },
            initrd: Artifact { path: "initramfs/initramfs.cpio.gz".into(), sha256: [0; 32] },
            cvm: Cvm {
                cmdline: KernelCommandLine("panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}".into()),
                images: CvmImages {
                    cpu: CvmImage {
                        disk: CvmDisk {
                            artifact: Artifact { path: "vm_images/cvm-cpu.qcow2".into(), sha256: [0; 32] },
                            format: DiskFormat::Qcow2,
                        },
                        verity: Verity {
                            disk: VerityDisk {
                                path: "vm_images/cvm-cpu-verity/verity-hash-dev".into(),
                                format: DiskFormat::Raw,
                            },
                            root_hash: meta.cpu_verity_root_hash,
                        },
                        kernel: Artifact { path: "vm_images/kernel/cpu-vmlinuz".into(), sha256: [0; 32] },
                    },
                    gpu: CvmImage {
                        disk: CvmDisk {
                            artifact: Artifact { path: "vm_images/cvm-gpu.qcow2".into(), sha256: [0; 32] },
                            format: DiskFormat::Qcow2,
                        },
                        verity: Verity {
                            disk: VerityDisk {
                                path: "vm_images/cvm-gpu-verity/verity-hash-dev".into(),
                                format: DiskFormat::Raw,
                            },
                            root_hash: meta.gpu_verity_root_hash,
                        },
                        kernel: Artifact { path: "vm_images/kernel/gpu-vmlinuz".into(), sha256: [0; 32] },
                    },
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyMetadata {
    /// The CPU verity root hash.
    pub cpu_verity_root_hash: [u8; 32],

    /// The GPU verity root hash.
    pub gpu_verity_root_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageMetadata {
    /// The git commit that this package was built from.
    pub commit: String,
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
pub struct KernelCommandLine(String);

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        let input = r#"{ "kernel": { "commit": "e8b814d629a0c2073239828e63d50b125c013570" }, "qemu": { "commit": "e8b814d629a0c2073239828e63d50b125c013570" }, "ovmf": { "path": "vm_images/ovmf/OVMF.fd", "sha256": "e842c3c58a54172592f6345ae7f44b1c7fe7c76af9578765932a77674e4475bb" }, "initrd": { "path": "initramfs/initramfs.cpio.gz", "sha256": "115c25046d4f357edae4506be45c5c6a79543a15db1b4ef2ec94bcfe70bd66ec" }, "cvm": { "cmdline": "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}", "images": { "cpu": { "disk": { "path": "vm_images/cvm-cpu.qcow2", "format": "qcow2", "sha256": "1287785f6a6a2cb08f0c25b864f8976a76e7e0d2a7c906738e2bc374266b3708" }, "verity": { "disk": { "path": "vm_images/cvm-cpu-verity/verity-hash-dev", "format": "raw" }, "root_hash": "4324eabc4d0d9aa2aed99f7ac16bd118473f455ab97841674afd3bc318d755cb" }, "kernel": { "path": "vm_images/kernel/cpu-vmlinuz", "sha256": "92cefd4d94338ad808d3c1a1be3b1d166d92a654fcb52aecdbe8905e7970817e" } }, "gpu": { "disk": { "path": "vm_images/cvm-gpu.qcow2", "format": "qcow2", "sha256": "1287785f6a6a2cb08f0c25b864f8976a76e7e0d2a7c906738e2bc374266b3708" }, "verity": { "disk": { "path": "vm_images/cvm-gpu-verity/verity-hash-dev", "format": "raw" }, "root_hash": "4324eabc4d0d9aa2aed99f7ac16bd118473f455ab97841674afd3bc318d755cb" }, "kernel": { "path": "vm_images/kernel/gpu-vmlinuz", "sha256": "92cefd4d94338ad808d3c1a1be3b1d166d92a654fcb52aecdbe8905e7970817e" } } } } }"#;
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
        let expected =  "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash=0000000000000000000000000000000000000000000000000000000000000000 state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash=aaa";
        assert_eq!(rendered, expected);
    }
}
