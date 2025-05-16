use docker_compose_types::{Compose, Ports};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File},
    io::{self, BufWriter},
    iter, mem,
    path::Path,
    process::{ExitStatus, Stdio},
    str::FromStr,
};
use tokio::{fs::create_dir_all, process::Command};
use tracing::info;

// The list of ports that can't be exported.
static RESERVED_PORTS: &[u16] = &[80, 443];

// The list of container names that are reserved.
static RESERVED_CONTAINERS: &[&str] = &["nilcc-attester", "nilcc-proxy"];

/// Information about the API container that will be the entrypoint to the VM image.
#[derive(Debug, Serialize)]
pub struct ContainerMetadata {
    /// The name of the container.
    pub container: String,

    /// The port at which to reach the container.
    pub port: u16,
}

/// The metadata for the application being ran.
#[derive(Debug, Serialize)]
pub struct ApplicationMetadata {
    /// The hostname to use for the TLS certificate exposed by this host.
    pub hostname: String,

    /// The entrypoint container information.
    pub api: ContainerMetadata,
}

/// The spec for the ISO being created.
#[derive(Debug)]
pub struct IsoSpec {
    /// The docker compose information.
    pub docker_compose_yaml: String,

    /// The application's metadata.
    pub metadata: ApplicationMetadata,
}

/// Allows creating ISOs.
pub struct IsoMaker;

impl IsoMaker {
    /// Create an ISO for an application to be run in a confidential VM.
    pub async fn create_application_iso(
        &self,
        spec: IsoSpec,
        iso_path: &Path,
    ) -> Result<IsoMetadata, ApplicationIsoError> {
        let IsoSpec { docker_compose_yaml, metadata } = spec;
        info!("Parsing docker compose YAML");
        let compose: Compose =
            serde_yaml::from_str(&docker_compose_yaml).map_err(ApplicationIsoError::YamlDeserialize)?;

        info!("Sanitizing docker compose");
        let compose = ComposeSanitizer { api: &metadata.api }.sanitize(compose)?;

        let tempdir = tempfile::TempDir::with_prefix("nilcc-agent").map_err(ApplicationIsoError::Tempdir)?;
        let input_path = tempdir.path().join("contents");
        create_dir_all(&input_path).await.map_err(ApplicationIsoError::FilesWrite)?;

        info!("Writing files into temporary directory: {}", input_path.display());
        let metadata = serde_json::to_string(&metadata)?;
        let compose_file =
            File::create(input_path.join("docker-compose.yaml")).map_err(ApplicationIsoError::FilesWrite)?;
        let compose_file = BufWriter::new(compose_file);
        serde_yaml::to_writer(compose_file, &compose).map_err(ApplicationIsoError::YamlSerialize)?;
        fs::write(input_path.join("metadata.json"), &metadata).map_err(ApplicationIsoError::FilesWrite)?;

        info!("Invoking mkisofs to generate ISO in {}", iso_path.display());
        let mut child = Command::new("mkisofs")
            .arg("-U")
            .arg("-o")
            .arg(iso_path)
            .arg(input_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ApplicationIsoError::SpawnMkisofs)?;
        let status = child.wait().await.map_err(ApplicationIsoError::RunningMkisofs)?;
        if !status.success() {
            return Err(ApplicationIsoError::MkisofsExit(status));
        }
        info!("ISO file generated at {}", iso_path.display());

        let metadata = IsoMetadata { docker_compose_hash: Sha256::digest(&docker_compose_yaml).into() };
        Ok(metadata)
    }
}

#[derive(Serialize)]
pub struct IsoMetadata {
    #[serde(with = "hex::serde")]
    docker_compose_hash: [u8; 32],
}

struct ComposeSanitizer<'a> {
    api: &'a ContainerMetadata,
}

impl ComposeSanitizer<'_> {
    fn sanitize(self, compose: Compose) -> Result<Compose, ComposeValidationError> {
        let compose = self.sanitize_services(compose)?;
        Ok(compose)
    }

    fn sanitize_services(&self, mut compose: Compose) -> Result<Compose, ComposeValidationError> {
        let mut found_api_container = false;
        for (name, spec) in &mut compose.services.0 {
            let spec = spec.as_mut().ok_or_else(|| ComposeValidationError::ServiceBodyMissing(name.clone()))?;
            // The container name is the one in `services:` and optionally also its
            // `container_name`, if any.
            let mut names = iter::once(name).chain(spec.container_name.as_ref());
            if let Some(reserved) = names.clone().find(|n| RESERVED_CONTAINERS.contains(&n.as_str())) {
                return Err(ComposeValidationError::ReservedContainerName(reserved.clone()));
            }

            if names.any(|n| *n == self.api.container) {
                found_api_container = true;
            }

            spec.ports = Self::sanitize_ports(mem::take(&mut spec.ports))?;
        }
        if !found_api_container {
            return Err(ComposeValidationError::ApiContainerMissing);
        }
        Ok(compose)
    }

    fn sanitize_ports(ports: Ports) -> Result<Ports, ComposeValidationError> {
        match ports {
            Ports::Short(ports) => {
                let mut sanitized_ports = Vec::new();
                for port in ports {
                    let mut parsed = ExposedPorts::from_str(&port)?;
                    if let Some(port) = parsed.exposed {
                        if RESERVED_PORTS.contains(&port) {
                            parsed.exposed = None;
                        }
                    }
                    sanitized_ports.push(parsed.to_string());
                }
                Ok(Ports::Short(sanitized_ports))
            }
            Ports::Long(mut ports) => {
                for port in &mut ports {
                    if let Some(published) = &port.published {
                        let published = PublishedPorts::try_from(published)?;
                        if let Some(reserved) = published.0.iter().find(|p| RESERVED_PORTS.contains(p)) {
                            if published.0.len() == 1 {
                                // If it's a single one we can take it out
                                port.published = None;
                            } else {
                                return Err(ComposeValidationError::PublishedPorts(*reserved));
                            }
                        }
                    }
                }
                Ok(Ports::Long(ports))
            }
        }
    }
}

struct PublishedPorts(Vec<u16>);

impl TryFrom<&docker_compose_types::PublishedPort> for PublishedPorts {
    type Error = ComposeValidationError;

    fn try_from(ports: &docker_compose_types::PublishedPort) -> Result<Self, Self::Error> {
        use docker_compose_types::PublishedPort::*;
        match ports {
            Single(port) => Ok(PublishedPorts(vec![*port])),
            Range(spec) => {
                let Some((from, to)) = spec.split_once("-") else {
                    return Err(ComposeValidationError::InvalidPorts);
                };
                let from: u16 = from.parse().map_err(|_| ComposeValidationError::InvalidPorts)?;
                let to: u16 = to.parse().map_err(|_| ComposeValidationError::InvalidPorts)?;
                if from > to {
                    return Err(ComposeValidationError::InvalidPorts);
                }
                Ok(PublishedPorts((from..=to).collect()))
            }
        }
    }
}

struct ExposedPorts {
    exposed: Option<u16>,
    target: u16,
}

impl FromStr for ExposedPorts {
    type Err = ComposeValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ComposeValidationError::InvalidPorts;
        let s = match s.split_once('/') {
            Some((s, protocol)) => {
                if !matches!(protocol, "udp" | "tcp") {
                    return Err(InvalidPorts);
                }
                s
            }
            None => s,
        };
        let (exposed, target) = match s.split_once(':') {
            Some((from, to)) => {
                let from = from.parse().map_err(|_| InvalidPorts)?;
                let to = to.parse().map_err(|_| InvalidPorts)?;
                (Some(from), to)
            }
            None => {
                let to = s.parse().map_err(|_| InvalidPorts)?;
                (None, to)
            }
        };
        Ok(Self { exposed, target })
    }
}

impl fmt::Display for ExposedPorts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(port) = self.exposed {
            write!(f, "{port}:")?;
        }
        write!(f, "{}", self.target)
    }
}

/// An error when creating an application ISO.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationIsoError {
    #[error("creating tempdir: {0}")]
    Tempdir(io::Error),

    #[error("YAML model deserialization: {0}")]
    YamlDeserialize(serde_yaml::Error),

    #[error("YAML model serialization: {0}")]
    YamlSerialize(serde_yaml::Error),

    #[error("JSON model serialization: {0}")]
    JsonSerialize(#[from] serde_json::Error),

    #[error("failed to write ISO files: {0}")]
    FilesWrite(io::Error),

    #[error("invalid docker compose: {0}")]
    Compose(#[from] ComposeValidationError),

    #[error("spawning mkisofs: {0}")]
    SpawnMkisofs(io::Error),

    #[error("running mkisofs: {0}")]
    RunningMkisofs(io::Error),

    #[error("mkisofs exited with error: {0}")]
    MkisofsExit(ExitStatus),
}

/// An error during the docker compose file validation.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ComposeValidationError {
    #[error("container cannot publish reserved port: {0}")]
    PublishedPorts(u16),

    #[error("container '{0}' cannot use reserved container name")]
    ReservedContainerName(String),

    #[error("api container not defined in docker compose")]
    ApiContainerMissing,

    #[error("no body for service {0}")]
    ServiceBodyMissing(String),

    #[error("invalid ports definition")]
    InvalidPorts,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::text(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - "hello"
"#
    )]
    #[case::too_many_colons(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - "80:80:80"
"#
    )]
    #[case::broken_range1(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - target: 80
        published: 80:79
"#
    )]
    fn malformed_docker_compose_ports(#[case] input: &str) {
        let compose: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        let err = ComposeSanitizer { api: &api }.sanitize(compose).expect_err("no failure");
        assert_eq!(err, ComposeValidationError::InvalidPorts);
    }

    #[rstest]
    #[case::no_container(
        ComposeValidationError::ApiContainerMissing,
        r#"
services:
  foo:
    image: foo:latest
"#
    )]
    #[case::exposed_ports_long(
        ComposeValidationError::PublishedPorts(443),
        r#"
services:
  api:
    image: foo:latest
    ports:
      - target: 80
        published: 443-444
"#
    )]
    #[case::reserved_name_top_level(
        ComposeValidationError::ReservedContainerName("nilcc-attester".to_string()),
        r#"
services:
  nilcc-attester:
    image: foo:latest
"#
    )]
    #[case::reserved_name_container_name(
        ComposeValidationError::ReservedContainerName("nilcc-proxy".to_string()),
        r#"
services:
  api:
    container_name: nilcc-proxy
    image: foo:latest
"#
    )]
    fn invalid_docker_composes(#[case] expected_error: ComposeValidationError, #[case] input: &str) {
        let compose: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        let err = ComposeSanitizer { api: &api }.sanitize(compose).expect_err("no failure");
        assert_eq!(err, expected_error);
    }

    #[rstest]
    #[case::exposed_ports_short(
        r#"
    services:
      api:
        image: foo:latest
        ports:
          - "443:443"
    "#,
        r#"
    services:
      api:
        image: foo:latest
        ports:
          - "443"
    "#
    )]
    #[case::exposed_ports_long(
        r#"
    services:
      api:
        image: foo:latest
        ports:
          - target: 80
            published: 80
    "#,
        r#"
    services:
      api:
        image: foo:latest
        ports:
          - target: 80
    "#
    )]
    fn sanitized_docker_composes(#[case] input: &str, #[case] output: &str) {
        let input: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let output: Compose = serde_yaml::from_str(output).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        let sanitized = ComposeSanitizer { api: &api }.sanitize(input).expect("validation failed");
        assert_eq!(sanitized, output);
    }

    #[rstest]
    #[case::service(
        r#"
services:
  api:
    image: foo:latest
"#
    )]
    #[case::container_name(
        r#"
services:
  foo:
    container_name: api
    image: foo:latest
"#
    )]
    #[case::publish_port(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - "81:81"
"#
    )]
    #[case::publish_port_long(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - target: 81
        published: 81-81
"#
    )]
    #[case::publish_port_many(
        r#"
services:
  api:
    image: foo:latest
    ports:
      - target: 81
        published: 81-100
"#
    )]
    fn valid_docker_composes(#[case] input: &str) {
        let compose: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        let sanitized = ComposeSanitizer { api: &api }.sanitize(compose.clone()).expect("validation failed");
        assert_eq!(sanitized, compose);
    }
}
