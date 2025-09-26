use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub build: u8,
}

#[cfg(feature = "sev")]
impl From<sev::firmware::guest::Version> for Version {
    fn from(version: sev::firmware::guest::Version) -> Self {
        let sev::firmware::guest::Version { major, minor, build } = version;
        Self { major, minor, build }
    }
}

#[cfg(feature = "sev")]
impl From<Version> for sev::firmware::guest::Version {
    fn from(version: Version) -> Self {
        let Version { major, minor, build } = version;
        Self { major, minor, build }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcbVersion {
    pub fmc: Option<u8>,
    pub bootloader: u8,
    pub tee: u8,
    pub snp: u8,
    pub microcode: u8,
}

#[cfg(feature = "sev")]
impl From<sev::firmware::host::TcbVersion> for TcbVersion {
    fn from(version: sev::firmware::host::TcbVersion) -> Self {
        let sev::firmware::host::TcbVersion { fmc, bootloader, tee, snp, microcode } = version;
        Self { fmc, bootloader, tee, snp, microcode }
    }
}

#[cfg(feature = "sev")]
impl From<TcbVersion> for sev::firmware::host::TcbVersion {
    fn from(version: TcbVersion) -> Self {
        let TcbVersion { fmc, bootloader, tee, snp, microcode } = version;
        Self { fmc, bootloader, tee, snp, microcode }
    }
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    #[serde_as(as = "[_; 72]")]
    pub r: [u8; 72],
    #[serde_as(as = "[_; 72]")]
    pub s: [u8; 72],
}

#[cfg(feature = "sev")]
impl From<sev::certs::snp::ecdsa::Signature> for Signature {
    fn from(signature: sev::certs::snp::ecdsa::Signature) -> Self {
        Self { r: *signature.r(), s: *signature.s() }
    }
}

#[cfg(feature = "sev")]
impl From<Signature> for sev::certs::snp::ecdsa::Signature {
    fn from(signature: Signature) -> Self {
        Self::new(signature.r, signature.s)
    }
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationReport {
    pub version: u32,
    pub guest_svn: u32,
    pub policy: u64,
    #[serde_as(as = "[_; 16]")]
    pub family_id: [u8; 16],
    #[serde_as(as = "[_; 16]")]
    pub image_id: [u8; 16],
    pub vmpl: u32,
    pub sig_algo: u32,
    pub current_tcb: TcbVersion,
    pub plat_info: u64,
    pub key_info: u32,
    #[serde_as(as = "[_; 64]")]
    pub report_data: [u8; 64],
    #[serde_as(as = "[_; 48]")]
    pub measurement: [u8; 48],
    pub host_data: [u8; 32],
    #[serde_as(as = "[_; 48]")]
    pub id_key_digest: [u8; 48],
    #[serde_as(as = "[_; 48]")]
    pub author_key_digest: [u8; 48],
    pub report_id: [u8; 32],
    pub report_id_ma: [u8; 32],
    pub reported_tcb: TcbVersion,
    pub cpuid_fam_id: Option<u8>,
    pub cpuid_mod_id: Option<u8>,
    pub cpuid_step: Option<u8>,
    #[serde_as(as = "[_; 64]")]
    pub chip_id: [u8; 64],
    pub committed_tcb: TcbVersion,
    pub current: Version,
    pub committed: Version,
    pub launch_tcb: TcbVersion,
    pub launch_mit_vector: Option<u64>,
    pub current_mit_vector: Option<u64>,
    pub signature: Signature,
}

#[cfg(feature = "sev")]
impl From<sev::firmware::guest::AttestationReport> for AttestationReport {
    fn from(report: sev::firmware::guest::AttestationReport) -> Self {
        let sev::firmware::guest::AttestationReport {
            version,
            guest_svn,
            policy,
            family_id,
            image_id,
            vmpl,
            sig_algo,
            current_tcb,
            plat_info,
            key_info,
            report_data,
            measurement,
            host_data,
            id_key_digest,
            author_key_digest,
            report_id,
            report_id_ma,
            reported_tcb,
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id,
            committed_tcb,
            current,
            committed,
            launch_tcb,
            launch_mit_vector,
            current_mit_vector,
            signature,
        } = report;
        Self {
            version,
            guest_svn,
            policy: policy.0,
            family_id,
            image_id,
            vmpl,
            sig_algo,
            current_tcb: current_tcb.into(),
            plat_info: plat_info.0,
            key_info: key_info.0,
            report_data,
            measurement,
            host_data,
            id_key_digest,
            author_key_digest,
            report_id,
            report_id_ma,
            reported_tcb: reported_tcb.into(),
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id,
            committed_tcb: committed_tcb.into(),
            current: current.into(),
            committed: committed.into(),
            launch_tcb: launch_tcb.into(),
            launch_mit_vector,
            current_mit_vector,
            signature: signature.into(),
        }
    }
}

#[cfg(feature = "sev")]
impl From<AttestationReport> for sev::firmware::guest::AttestationReport {
    fn from(report: AttestationReport) -> Self {
        let AttestationReport {
            version,
            guest_svn,
            policy,
            family_id,
            image_id,
            vmpl,
            sig_algo,
            current_tcb,
            plat_info,
            key_info,
            report_data,
            measurement,
            host_data,
            id_key_digest,
            author_key_digest,
            report_id,
            report_id_ma,
            reported_tcb,
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id,
            committed_tcb,
            current,
            committed,
            launch_tcb,
            launch_mit_vector,
            current_mit_vector,
            signature,
        } = report;
        Self {
            version,
            guest_svn,
            policy: policy.into(),
            family_id,
            image_id,
            vmpl,
            sig_algo,
            current_tcb: current_tcb.into(),
            plat_info: plat_info.into(),
            key_info: key_info.into(),
            report_data,
            measurement,
            host_data,
            id_key_digest,
            author_key_digest,
            report_id,
            report_id_ma,
            reported_tcb: reported_tcb.into(),
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id,
            committed_tcb: committed_tcb.into(),
            current: current.into(),
            committed: committed.into(),
            launch_tcb: launch_tcb.into(),
            launch_mit_vector,
            current_mit_vector,
            signature: signature.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde() {
        let json = r#"{"version":3,"guest_svn":0,"policy":196608,"family_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"image_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"vmpl":1,"sig_algo":1,"current_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"plat_info":5,"key_info":0,"report_data":[0,60,221,26,64,207,190,76,225,105,36,230,108,147,53,33,109,126,113,156,220,216,185,215,175,255,37,200,229,104,105,56,190,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"measurement":[133,218,39,154,206,134,74,150,158,59,191,220,170,182,127,243,1,116,2,183,210,43,162,65,82,155,29,74,121,185,166,148,43,137,212,230,218,103,71,200,1,226,114,104,50,85,174,75],"host_data":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"id_key_digest":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"author_key_digest":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"report_id":[141,246,201,117,45,5,37,129,218,56,1,145,154,84,173,74,69,142,27,48,209,37,52,206,67,156,46,182,39,2,34,101],"report_id_ma":[255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255],"reported_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"cpuid_fam_id":25,"cpuid_mod_id":17,"cpuid_step":1,"chip_id":[83,104,118,136,195,115,97,41,69,129,48,75,25,36,41,224,178,186,159,238,226,71,206,49,193,30,136,232,198,195,91,251,64,179,47,143,154,245,53,8,126,216,226,55,111,186,64,60,15,33,178,120,21,11,189,96,164,173,48,229,145,223,253,109],"committed_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"current":{"major":1,"minor":55,"build":39},"committed":{"major":1,"minor":55,"build":39},"launch_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"signature":{"r":[117,157,183,45,56,121,141,255,189,223,139,156,55,142,7,194,168,53,111,152,56,159,90,145,196,95,77,32,129,207,84,100,229,238,49,93,144,144,125,106,116,72,22,205,181,27,14,233,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"s":[153,219,194,30,7,26,10,68,95,157,246,221,70,91,217,116,69,100,124,253,96,120,235,241,153,215,136,80,25,142,0,203,189,184,52,117,209,56,125,230,187,38,187,244,100,230,18,97,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#;
        let report1: AttestationReport = serde_json::from_str(json).expect("deserialization failed");
        let serialized = serde_json::to_string(&report1).expect("serialization failed");
        let report2: AttestationReport = serde_json::from_str(&serialized).expect("deserialization failed");
        assert_eq!(report1, report2);
    }
}
