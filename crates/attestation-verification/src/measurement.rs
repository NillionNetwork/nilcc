use nilcc_artifacts::{
    VmType,
    metadata::{ArtifactsMetadata, KernelArgs, KernelCommandLine, MissingCommandLineParameter},
};
use sev::{
    error::MeasurementError,
    measurement::{
        snp::{SnpMeasurementArgs, snp_calc_launch_digest},
        vcpu_types::CpuType,
        vmsa::{GuestFeatures, VMMType},
    },
};
use std::path::{Path, PathBuf};
use tracing::info;

/// Generates the measurement that a CVM should have generated given the parameters.
pub struct MeasurementGenerator {
    pub vcpus: u32,
    pub ovmf: PathBuf,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub docker_compose_hash: [u8; 32],
    pub filesystem_root_hash: [u8; 32],
    pub kernel_args: KernelCommandLine,
}

impl MeasurementGenerator {
    pub fn new(
        docker_compose_hash: [u8; 32],
        vcpus: u32,
        vm_type: VmType,
        metadata: &ArtifactsMetadata,
        artifacts_path: &Path,
    ) -> Self {
        let vm_type_metadata = metadata.cvm.images.resolve(vm_type);
        Self {
            vcpus,
            ovmf: artifacts_path.join(&metadata.ovmf.path),
            kernel: artifacts_path.join(&vm_type_metadata.kernel.path),
            initrd: artifacts_path.join(&metadata.initrd.path),
            docker_compose_hash,
            filesystem_root_hash: vm_type_metadata.verity.root_hash,
            kernel_args: metadata.cvm.cmdline.clone(),
        }
    }

    pub fn generate(self) -> Result<Vec<u8>, MeasurementHashError> {
        let Self { ovmf, kernel, initrd, docker_compose_hash, filesystem_root_hash, vcpus, kernel_args } = self;
        let docker_compose_hash = hex::encode(docker_compose_hash);
        let cmdline = kernel_args.render(KernelArgs {
            docker_compose_hash: &docker_compose_hash,
            filesystem_root_hash: &filesystem_root_hash,
        })?;
        info!("Using kernel parameters for measurement: {cmdline}");
        let guest_features = GuestFeatures(0x01);
        let args = SnpMeasurementArgs {
            vcpus,
            vcpu_type: CpuType::EpycV4,
            ovmf_file: ovmf,
            guest_features,
            kernel_file: Some(kernel),
            initrd_file: Some(initrd),
            append: Some(&cmdline),
            ovmf_hash_str: None,
            vmm_type: Some(VMMType::QEMU),
        };
        let digest: Vec<u8> = snp_calc_launch_digest(args)?.try_into()?;
        Ok(digest)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MeasurementHashError {
    #[error("generating measurement hash: {0}")]
    Measurement(#[from] MeasurementError),

    #[error(transparent)]
    KernelArgs(#[from] MissingCommandLineParameter),
}
