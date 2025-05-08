use docker_compose_types::{Compose, Ports};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::Path,
    process::{ExitStatus, Stdio},
    str::FromStr,
};
use tokio::{fs::create_dir_all, process::Command};
use tracing::info;

// The list of ports that can't be exported.
static RESERVED_PORTS: &[u16] = &[443];

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
        let compose: Compose = serde_yaml::from_str(&docker_compose_yaml)?;

        info!("Validating docker compose");
        ComposeValidator { compose, api: &metadata.api }.validate()?;

        let tempdir = tempfile::TempDir::with_prefix("nilcc-agent").map_err(ApplicationIsoError::Tempdir)?;
        let input_path = tempdir.path().join("contents");
        create_dir_all(&input_path).await.map_err(ApplicationIsoError::FilesWrite)?;

        info!("Writing files into temporary directory: {}", input_path.display());
        let metadata = serde_json::to_string(&metadata)?;
        fs::write(input_path.join("docker-compose.yaml"), &docker_compose_yaml)
            .map_err(ApplicationIsoError::FilesWrite)?;
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

struct ComposeValidator<'a> {
    compose: Compose,
    api: &'a ContainerMetadata,
}

impl ComposeValidator<'_> {
    fn validate(self) -> Result<(), ComposeValidationError> {
        self.validate_services()?;
        Ok(())
    }

    fn validate_services(&self) -> Result<(), ComposeValidationError> {
        let mut found_api_container = false;
        for (name, spec) in &self.compose.services.0 {
            let spec = spec.as_ref().ok_or_else(|| ComposeValidationError::ServiceBodyMissing(name.clone()))?;
            if name == &self.api.container || spec.container_name.as_deref() == Some(&self.api.container) {
                found_api_container = true;
            }
            Self::validate_ports(&spec.ports)?;
        }
        if !found_api_container {
            return Err(ComposeValidationError::ApiContainerMissing);
        }
        Ok(())
    }

    fn validate_ports(ports: &Ports) -> Result<(), ComposeValidationError> {
        let ports: Vec<u16> = match ports {
            Ports::Short(ports) => ports
                .iter()
                .flat_map(|port| ExposedPorts::from_str(port).map(|p| p.exposed).transpose())
                .collect::<Result<_, _>>()?,
            Ports::Long(ports) => {
                let all_ports: Vec<_> = ports
                    .iter()
                    .flat_map(|p| &p.published)
                    .map(|p| PublishedPorts::try_from(p).map(|p| p.0))
                    .collect::<Result<_, _>>()?;
                all_ports.into_iter().flatten().collect()
            }
        };

        for port in ports {
            if RESERVED_PORTS.contains(&port) {
                return Err(ComposeValidationError::PublishedPorts(port));
            }
        }
        Ok(())
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
                let Some((from, to)) = spec.split_once(":") else {
                    return Err(ComposeValidationError::InvalidPorts);
                };
                let from: u16 = from.parse().map_err(|_| ComposeValidationError::InvalidPorts)?;
                let to: u16 = to.parse().map_err(|_| ComposeValidationError::InvalidPorts)?;
                Ok(PublishedPorts((from..=to).collect()))
            }
        }
    }
}

struct ExposedPorts {
    exposed: Option<u16>,
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
        let exposed = match s.split_once(':') {
            Some((from, _)) => Some(from.parse().map_err(|_| InvalidPorts)?),
            None => None,
        };
        Ok(Self { exposed })
    }
}

/// An error when creating an application ISO.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationIsoError {
    #[error("creating tempdir: {0}")]
    Tempdir(io::Error),

    #[error("YAML model deserialization: {0}")]
    YamlDeserialize(#[from] serde_yaml::Error),

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
    #[case::no_container(
        ComposeValidationError::ApiContainerMissing,
        r#"
services:
  foo:
    image: foo:latest
"#
    )]
    #[case::exposed_ports_short(
        ComposeValidationError::PublishedPorts(443),
        r#"
services:
  api:
    image: foo:latest
    ports:
      - "443:80"
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
        published: 443
"#
    )]
    fn invalid_docker_composes(#[case] expected_error: ComposeValidationError, #[case] input: &str) {
        let compose: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        let err = ComposeValidator { compose, api: &api }.validate().expect_err("no failure");
        assert_eq!(err, expected_error);
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
      - "80:80"
"#
    )]
    fn valid_docker_composes(#[case] input: &str) {
        let compose: Compose = serde_yaml::from_str(input).expect("invalid docker compose");
        let api = ContainerMetadata { container: "api".to_string(), port: 80 };
        ComposeValidator { compose, api: &api }.validate().expect("validation failed");
    }
}
