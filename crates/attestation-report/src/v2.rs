use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;
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
    #[serde_as(as = "Hex")]
    pub r: [u8; 72],
    #[serde_as(as = "Hex")]
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
        // The `Array` type is not exposed so we are forced to use a fallible conversion, even
        // though we know we have the right array size
        Self::new(
            signature.r.try_into().expect("signature conversion"),
            signature.s.try_into().expect("signature conversion"),
        )
    }
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationReport {
    pub version: u32,
    pub guest_svn: u32,
    pub policy: u64,
    #[serde_as(as = "Hex")]
    pub family_id: [u8; 16],
    #[serde_as(as = "Hex")]
    pub image_id: [u8; 16],
    pub vmpl: u32,
    pub sig_algo: u32,
    pub current_tcb: TcbVersion,
    pub plat_info: u64,
    pub key_info: u32,
    #[serde_as(as = "Hex")]
    pub report_data: [u8; 64],
    #[serde_as(as = "Hex")]
    pub measurement: [u8; 48],
    #[serde_as(as = "Hex")]
    pub host_data: [u8; 32],
    #[serde_as(as = "Hex")]
    pub id_key_digest: [u8; 48],
    #[serde_as(as = "Hex")]
    pub author_key_digest: [u8; 48],
    #[serde_as(as = "Hex")]
    pub report_id: [u8; 32],
    #[serde_as(as = "Hex")]
    pub report_id_ma: [u8; 32],
    pub reported_tcb: TcbVersion,
    pub cpuid_fam_id: Option<u8>,
    pub cpuid_mod_id: Option<u8>,
    pub cpuid_step: Option<u8>,
    #[serde_as(as = "Hex")]
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
            family_id: family_id.0,
            image_id: image_id.0,
            vmpl,
            sig_algo,
            current_tcb: current_tcb.into(),
            plat_info: plat_info.0,
            key_info: key_info.0,
            report_data: report_data.0,
            measurement: measurement.0,
            host_data: host_data.0,
            id_key_digest: id_key_digest.0,
            author_key_digest: author_key_digest.0,
            report_id: report_id.0,
            report_id_ma: report_id_ma.0,
            reported_tcb: reported_tcb.into(),
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id: chip_id.0,
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
            family_id: family_id.try_into().unwrap(),
            image_id: image_id.try_into().unwrap(),
            vmpl,
            sig_algo,
            current_tcb: current_tcb.into(),
            plat_info: plat_info.into(),
            key_info: key_info.into(),
            report_data: report_data.try_into().unwrap(),
            measurement: measurement.try_into().unwrap(),
            host_data: host_data.try_into().unwrap(),
            id_key_digest: id_key_digest.try_into().unwrap(),
            author_key_digest: author_key_digest.try_into().unwrap(),
            report_id: report_id.try_into().unwrap(),
            report_id_ma: report_id_ma.try_into().unwrap(),
            reported_tcb: reported_tcb.into(),
            cpuid_fam_id,
            cpuid_mod_id,
            cpuid_step,
            chip_id: chip_id.try_into().unwrap(),
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
        let json = r#"{"version":3,"guest_svn":0,"policy":196608,"family_id":"00000000000000000000000000000000","image_id":"00000000000000000000000000000000","vmpl":1,"sig_algo":1,"current_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"plat_info":5,"key_info":0,"report_data":"003cdd1a40cfbe4ce16924e66c9335216d7e719cdcd8b9d7afff25c8e5686938be00000000000000000000000000000000000000000000000000000000000000","measurement":"85da279ace864a969e3bbfdcaab67ff3017402b7d22ba241529b1d4a79b9a6942b89d4e6da6747c801e272683255ae4b","host_data":"0000000000000000000000000000000000000000000000000000000000000000","id_key_digest":"000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","author_key_digest":"000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","report_id":"8df6c9752d052581da3801919a54ad4a458e1b30d12534ce439c2eb627022265","report_id_ma":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","reported_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"cpuid_fam_id":25,"cpuid_mod_id":17,"cpuid_step":1,"chip_id":"53687688c37361294581304b192429e0b2ba9feee247ce31c11e88e8c6c35bfb40b32f8f9af535087ed8e2376fba403c0f21b278150bbd60a4ad30e591dffd6d","committed_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"current":{"major":1,"minor":55,"build":39},"committed":{"major":1,"minor":55,"build":39},"launch_tcb":{"fmc":null,"bootloader":9,"tee":0,"snp":23,"microcode":72},"signature":{"r":"759db72d38798dffbddf8b9c378e07c2a8356f98389f5a91c45f4d2081cf5464e5ee315d90907d6a744816cdb51b0ee9000000000000000000000000000000000000000000000000","s":"99dbc21e071a0a445f9df6dd465bd97445647cfd6078ebf199d78850198e00cbbdb83475d1387de6bb26bbf464e61261000000000000000000000000000000000000000000000000"}}"#;
        let report1: AttestationReport = serde_json::from_str(json).expect("deserialization failed");
        let serialized = serde_json::to_string(&report1).expect("serialization failed");
        let report2: AttestationReport = serde_json::from_str(&serialized).expect("deserialization failed");
        assert_eq!(report1, report2);
    }
}
