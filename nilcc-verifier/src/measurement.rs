use anyhow::Context;
use sev::measurement::{
    snp::{snp_calc_launch_digest, SnpMeasurementArgs},
    vcpu_types::CpuType,
    vmsa::{GuestFeatures, VMMType},
};
use std::path::PathBuf;

/// Generates the measurement that a CVM should have generated given the parameters.
pub struct MeasurementGenerator {
    pub vcpus: u32,
    pub ovmf: PathBuf,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub docker_compose_hash: String,
    pub filesystem_root_hash: String,
    pub kernel_debug_options: bool,
}

impl MeasurementGenerator {
    pub fn generate(self) -> anyhow::Result<Vec<u8>> {
        let Self {
            ovmf,
            kernel,
            initrd,
            docker_compose_hash,
            filesystem_root_hash,
            vcpus,
            kernel_debug_options: debug_options,
        } = self;
        let debug_options = match debug_options {
            true => "console=ttyS0 earlyprintk=serial panic=-1 ",
            false => "",
        };
        let cmd_line = format!("{debug_options}root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={filesystem_root_hash} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={docker_compose_hash}");
        let guest_features = GuestFeatures(0x01);
        let args = SnpMeasurementArgs {
            vcpus,
            vcpu_type: CpuType::EpycV4,
            ovmf_file: ovmf,
            guest_features,
            kernel_file: Some(kernel),
            initrd_file: Some(initrd),
            append: Some(&cmd_line),
            ovmf_hash_str: None,
            vmm_type: Some(VMMType::QEMU),
        };
        let digest = snp_calc_launch_digest(args).context("generating SNP measurement")?;
        let digest = bincode::serialize(&digest).context("bindecoding SNP measurement")?;
        Ok(digest)
    }
}
