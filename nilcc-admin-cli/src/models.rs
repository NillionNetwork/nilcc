use serde::Serialize;
use uuid::Uuid;

pub mod accounts {
    use super::*;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CreateAccountRequest {
        pub name: String,
        pub credits: u64,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AddCreditsRequest {
        pub account_id: Uuid,
        pub credits: u64,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct UpdateAccountRequest {
        pub account_id: Uuid,
        pub name: String,
    }
}

pub mod tiers {
    use super::*;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CreateTierRequest {
        pub name: String,
        pub cost: u64,
        pub cpus: u64,
        pub gpus: u64,
        pub memory_mb: u64,
        pub disk_gb: u64,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DeleteTierRequest {
        pub tier_id: Uuid,
    }
}

pub mod artifacts {
    use super::*;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct EnableArtifactVersionRequest {
        pub version: String,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DisableArtifactVersionRequest {
        pub version: String,
    }
}

pub mod metal_instances {
    use super::*;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DeleteMetalInstanceRequest {
        pub metal_instance_id: Uuid,
    }
}
