use nilcc_artifacts::metadata::{KernelArgs, KernelCommandLine, MissingCommandLineParameter};
use sev::{
    error::MeasurementError,
    measurement::{
        snp::{SnpMeasurementArgs, snp_calc_launch_digest},
        vcpu_types::CpuType,
        vmsa::{GuestFeatures, VMMType},
    },
};
use std::path::PathBuf;
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
