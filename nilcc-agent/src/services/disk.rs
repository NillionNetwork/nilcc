use crate::clients::qemu::HardDiskFormat;
use anyhow::{bail, Context};
use async_trait::async_trait;
use serde::Serialize;
use std::{
    fmt::Write,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    str::FromStr,
};
use tokio::{
    fs::{self, create_dir_all},
    process::Command,
};
use tracing::info;

/// The list of reserved environment variable names.
static RESERVED_ENVIRONMENT_VARIABLES: &[&str] = &["NILCC_VERSION", "NILCC_VM_TYPE"];

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DiskService: Send + Sync {
    /// Create a disk at the given path with the given format.
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: u32) -> anyhow::Result<()>;

    /// Create the ISO for an application.
    async fn create_application_iso(&self, path: &Path, spec: IsoSpec) -> Result<(), CreateIsoError>;
}

pub struct DefaultDiskService {
    qemu_img_path: PathBuf,
}

impl DefaultDiskService {
    pub fn new(qemu_img_path: PathBuf) -> Self {
        Self { qemu_img_path }
    }

    fn serialize_environment_variables(variables: &[EnvironmentVariable]) -> String {
        let mut output = String::new();
        for EnvironmentVariable { name, value } in variables {
            writeln!(output, "{name}={value}").expect("cannot happen");
        }
        output
    }
}

#[async_trait]
impl DiskService for DefaultDiskService {
    async fn create_disk(&self, path: &Path, format: HardDiskFormat, size_gib: u32) -> anyhow::Result<()> {
        let format = format.to_string();
        let args = ["create", "-f", &format, &path.to_string_lossy(), &format!("{size_gib}G")];
        let output = Command::new(&self.qemu_img_path)
            .args(args)
            .output()
            .await
            .context("Failed to invoke qemu-img (is qemu-img path correct?)")?;
        if output.status.success() {
            Ok(())
        } else {
            bail!("qemu-img failed: {}", String::from_utf8_lossy(&output.stderr))
        }
    }

    async fn create_application_iso(&self, path: &Path, spec: IsoSpec) -> Result<(), CreateIsoError> {
        use CreateIsoError::*;
        let IsoSpec { docker_compose_yaml, metadata, environment_variables } = spec;

        // Make sure no reserved environment variable names are used.
        if let Some(var) =
            environment_variables.iter().find(|e| RESERVED_ENVIRONMENT_VARIABLES.contains(&e.name.as_str()))
        {
            return Err(ReservedEnvironmentVariable(var.name.clone()));
        }

        let tempdir = tempfile::TempDir::with_prefix("nilcc-agent").map_err(Tempdir)?;
        let input_path = tempdir.path().join("contents");
        create_dir_all(&input_path).await.map_err(FilesWrite)?;

        info!("Writing files into temporary directory: {}", input_path.display());
        let metadata = serde_json::to_string(&metadata)?;
        fs::write(input_path.join("docker-compose.yaml"), &docker_compose_yaml).await.map_err(FilesWrite)?;
        fs::write(input_path.join("metadata.json"), &metadata).await.map_err(FilesWrite)?;

        let variables = Self::serialize_environment_variables(&environment_variables);
        fs::write(input_path.join(".env"), &variables).await.map_err(FilesWrite)?;

        info!("Invoking mkisofs to generate ISO in {}", path.display());
        let mut child = Command::new("mkisofs")
            .arg("-U")
            .arg("-o")
            .arg(path)
            .arg(input_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(SpawnMkisofs)?;
        let status = child.wait().await.map_err(RunningMkisofs)?;
        if !status.success() {
            return Err(MkisofsExit(status));
        }
        info!("ISO file generated at {}", path.display());
        Ok(())
    }
}

/// An environment variable.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentVariable {
    /// The environment variable name.
    pub name: String,

    /// The environment variable value.
    pub value: String,
}

impl EnvironmentVariable {
    pub fn new<S: Into<String>>(name: S, value: S) -> Self {
        Self { name: name.into(), value: value.into() }
    }
}

impl FromStr for EnvironmentVariable {
    type Err = ParseEnvironmentVariableError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, value) = s.split_once('=').ok_or(ParseEnvironmentVariableError)?;
        let name = name.trim().to_string();
        let value = value.to_string();
        Ok(Self { name, value })
    }
}

/// An error returned when parsing an environment variable.
#[derive(Debug, thiserror::Error)]
#[error("expected environment variable in <name>=<value> syntax")]
pub struct ParseEnvironmentVariableError;

/// Information about the API container that will be the entrypoint to the VM image.
#[derive(Debug, Serialize, PartialEq)]
pub struct ContainerMetadata {
    /// The name of the container.
    pub container: String,

    /// The port at which to reach the container.
    pub port: u16,
}

/// The metadata for the application being ran.
#[derive(Debug, Serialize, PartialEq)]
pub struct ApplicationMetadata {
    /// The hostname to use for the TLS certificate exposed by this host.
    pub hostname: String,

    /// The entrypoint container information.
    pub api: ContainerMetadata,
}

/// The spec for the ISO being created.
#[derive(Debug, PartialEq)]
pub struct IsoSpec {
    /// The docker compose information.
    pub docker_compose_yaml: String,

    /// The application's metadata.
    pub metadata: ApplicationMetadata,

    /// The environment variables to be passed to the running containers.
    pub environment_variables: Vec<EnvironmentVariable>,
}

/// An error when creating an application ISO.
#[derive(Debug, thiserror::Error)]
pub enum CreateIsoError {
    #[error("creating tempdir: {0}")]
    Tempdir(io::Error),

    #[error("JSON model serialization: {0}")]
    JsonSerialize(#[from] serde_json::Error),

    #[error("failed to write ISO files: {0}")]
    FilesWrite(io::Error),

    #[error("spawning mkisofs: {0}")]
    SpawnMkisofs(io::Error),

    #[error("running mkisofs: {0}")]
    RunningMkisofs(io::Error),

    #[error("mkisofs exited with error: {0}")]
    MkisofsExit(ExitStatus),

    #[error("environment variable '{0}' is reserved")]
    ReservedEnvironmentVariable(String),
}
