use convert_case::{Case, Casing};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::serde_as;
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;
use validator::{Validate, ValidationError};

pub mod workloads {
    use super::*;

    pub mod create {
        use super::*;

        static FILENAME_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\w/._-]+$").unwrap());
        static DOMAIN_REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9-\.]+\.([a-zA-Z]{2,}|[a-zA-Z]{2,}\.[a-zA-Z]{2,})$").unwrap());

        fn validate_files(files: &HashMap<String, Vec<u8>>) -> Result<(), ValidationError> {
            for key in files.keys() {
                if !FILENAME_REGEX.is_match(key) {
                    return Err(ValidationError::new("invalid filename"));
                }
            }
            Ok(())
        }

        #[serde_as]
        #[derive(Clone, Debug, Serialize, Deserialize, Validate)]
        #[serde(rename_all = "camelCase")]
        pub struct CreateWorkloadRequest {
            pub id: Uuid,

            pub docker_compose: String,

            #[serde(default)]
            pub env_vars: HashMap<String, String>,

            #[serde_as(as = "HashMap<_, Base64>")]
            #[serde(default)]
            #[validate(custom(function = "validate_files"))]
            pub files: HashMap<String, Vec<u8>>,

            pub public_container_name: String,

            pub public_container_port: u16,

            #[validate(range(min = 512))]
            pub memory_mb: u32,

            #[validate(range(min = 1))]
            pub cpus: u32,

            pub gpus: u16,

            #[validate(range(min = 2))]
            pub disk_space_gb: u32,

            #[validate(regex(path  = DOMAIN_REGEX))]
            pub domain: String,
        }

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct CreateWorkloadResponse {
            pub id: Uuid,
        }
    }

    pub mod list {
        use super::*;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ListWorkloadsRequest {
            pub id: Uuid,
        }

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct WorkloadSummary {
            pub id: Uuid,
            pub enabled: bool,
            pub domain: String,
        }
    }

    pub mod delete {
        use super::*;

        #[derive(Clone, Debug, Serialize, Deserialize, Validate)]
        #[serde(rename_all = "camelCase")]
        pub struct DeleteWorkloadRequest {
            pub id: Uuid,
        }
    }

    pub mod start {
        use super::*;

        #[derive(Clone, Debug, Serialize, Deserialize, Validate)]
        #[serde(rename_all = "camelCase")]
        pub struct StartWorkloadRequest {
            pub id: Uuid,
        }
    }

    pub mod stop {
        use super::*;

        #[derive(Clone, Debug, Serialize, Deserialize, Validate)]
        #[serde(rename_all = "camelCase")]
        pub struct StopWorkloadRequest {
            pub id: Uuid,
        }
    }

    pub mod restart {
        use super::*;

        #[derive(Clone, Debug, Serialize, Deserialize, Validate)]
        #[serde(rename_all = "camelCase")]
        pub struct RestartWorkloadRequest {
            pub id: Uuid,
        }
    }
}

pub mod errors {
    use super::*;

    /// An error when handling a request.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RequestHandlerError {
        /// A descriptive message about the error that was encountered.
        pub message: String,

        /// The error code.
        pub error_code: String,
    }

    impl RequestHandlerError {
        pub fn new(message: impl Into<String>, error_code: impl AsRef<str>) -> Self {
            let error_code = error_code.as_ref().to_case(Case::UpperSnake);
            Self { message: message.into(), error_code }
        }
    }
}
