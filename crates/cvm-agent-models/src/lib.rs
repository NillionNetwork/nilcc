use serde::{Deserialize, Serialize};
use validator::Validate;

pub mod container {
    use super::*;

    /// A container.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Container {
        /// The names for this container.
        pub names: Vec<String>,

        /// The container image.
        pub image: String,

        /// The container image id.
        pub image_id: String,

        /// The state of this container.
        pub state: String,
    }
}

pub mod logs {
    use super::*;

    /// A request to get the logs for a container.
    #[derive(Deserialize, Serialize, Validate)]
    #[serde(rename_all = "camelCase")]
    pub struct ContainerLogsRequest {
        /// The container that we're pulling logs out of.
        pub container: String,

        /// Whether to pull logs from the tail of the stream.
        pub tail: bool,

        /// The stream to take logs out of.
        pub stream: OutputStream,

        /// The maximum number of log lines to be returned.
        #[validate(range(max = 1000))]
        pub max_lines: usize,
    }

    /// The stream to take logs out of.
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum OutputStream {
        /// Standard output.
        Stdout,

        /// Standard error.
        Stderr,
    }

    /// The container logs response.
    #[derive(Deserialize, Serialize)]
    pub struct ContainerLogsResponse {
        pub lines: Vec<String>,
    }
}
