use anyhow::{Context, bail};
use async_trait::async_trait;
use cvm_agent_models::bootstrap::{CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use nilcc_artifacts::metadata::DiskFormat;
use serde::Serialize;
use std::{
    fmt::Write,
    io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};
use tokio::{
    fs::{self, create_dir_all},
    process::Command,
};
use tracing::info;

/// The list of reserved environment variable names.
static RESERVED_ENVIRONMENT_VARIABLES: &[&str] = &[
    "NILCC_VERSION",
    "NILCC_VM_TYPE",
    "NILCC_DOMAIN",
    "FILES",
    "CADDY_INPUT_FILE",
    CADDY_ACME_EAB_KEY_ID,
    CADDY_ACME_EAB_MAC_KEY,
];

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DiskService: Send + Sync {
    /// Create a disk at the given path with the given format.
    async fn create_disk(&self, path: &Path, format: DiskFormat, size_gib: u32) -> anyhow::Result<()>;

    /// Create a qemu disk snapshot.
    async fn create_qcow2_snapshot(&self, target: &Path, origin: &Path) -> anyhow::Result<()>;

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

    async fn persist_files(&self, base_path: &Path, files: Vec<ExternalFile>) -> Result<(), CreateIsoError> {
        use CreateIsoError::*;
        let base_path = base_path.canonicalize().map_err(FilesWrite)?.join("files");
        fs::create_dir(&base_path).await.map_err(FilesWrite)?;
        for file in files {
            // ensure no '..'
            if file.name.contains("..") {
                return Err(RelativePath(file.name));
            }
            let target_path = base_path.join(&file.name);
            // ensure it's still relative, e.g. `file.name` could have been /etc/password
            if !target_path.starts_with(&base_path) {
                return Err(RelativePath(file.name));
            }
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).await.map_err(FilesWrite)?;
            }
            fs::write(target_path, &file.contents).await.map_err(FilesWrite)?;
        }
        Ok(())
    }

    async fn qemu_img(&self, args: &[&str]) -> anyhow::Result<()> {
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
}

#[async_trait]
impl DiskService for DefaultDiskService {
    async fn create_disk(&self, path: &Path, format: DiskFormat, size_gib: u32) -> anyhow::Result<()> {
        let format = format.to_string();
        let args = ["create", "-f", &format, &path.to_string_lossy(), &format!("{size_gib}G")];
        self.qemu_img(&args).await
    }

    async fn create_qcow2_snapshot(&self, target: &Path, origin: &Path) -> anyhow::Result<()> {
        let format = "qcow2";
        let args = ["create", "-f", format, "-b", &origin.to_string_lossy(), "-F", format, &target.to_string_lossy()];
        self.qemu_img(&args).await
    }

    async fn create_application_iso(&self, path: &Path, spec: IsoSpec) -> Result<(), CreateIsoError> {
        use CreateIsoError::*;
        let IsoSpec { docker_compose_yaml, metadata, environment_variables, files } = spec;

        // Make sure no reserved environment variable names are used.
        if let Some(var) =
            environment_variables.iter().find(|e| RESERVED_ENVIRONMENT_VARIABLES.contains(&e.name.as_str()))
        {
            return Err(ReservedEnvironmentVariable(var.name.clone()));
        }

        let tempdir = tempfile::TempDir::with_prefix("nilcc-agent").map_err(Tempdir)?;
        let input_path = tempdir.path().join("contents");
        create_dir_all(&input_path).await.map_err(FilesWrite)?;
        self.persist_files(&input_path, files).await?;

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

/// The contents of a file to be written into the ISO.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalFile {
    /// The filename.
    pub name: String,

    /// The contents of this file.
    pub contents: Vec<u8>,
}

impl ExternalFile {
    pub fn new<S: Into<String>, B: Into<Vec<u8>>>(name: S, contents: B) -> Self {
        Self { name: name.into(), contents: contents.into() }
    }
}

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

    /// The files to be accessible in the docker compose.
    pub files: Vec<ExternalFile>,
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

    #[error("invalid relative path: {0}")]
    RelativePath(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::tempdir;

    fn make_service() -> DefaultDiskService {
        DefaultDiskService::new("".into())
    }

    #[tokio::test]
    async fn persist_files() {
        let service = make_service();
        let files = vec![
            ExternalFile { name: "foo.txt".into(), contents: b"hi".into() },
            ExternalFile { name: "bar/tar.txt".into(), contents: b"bye".into() },
        ];
        let workdir = tempdir().expect("failed to create tempdir");
        let base_path = workdir.path();
        service.persist_files(&base_path, files).await.expect("failed to persist");

        assert_eq!(std::fs::read_to_string(base_path.join("files/foo.txt")).expect("failed to read"), "hi");
        assert_eq!(std::fs::read_to_string(base_path.join("files/bar/tar.txt")).expect("failed to read"), "bye");
    }

    #[rstest]
    #[case::dot_dot1("../bar.txt")]
    #[case::dot_dot2("foo/../../bar.txt")]
    #[case::absolute("/etc/passwd")]
    #[tokio::test]
    async fn non_relative_file_paths(#[case] path: &str) {
        let service = make_service();
        let files = vec![ExternalFile { name: path.into(), contents: b"hi".into() }];
        let workdir = tempdir().expect("failed to create tempdir");
        let base_path = workdir.path();
        let err = service.persist_files(&base_path, files).await.expect_err("persist succeeded");
        assert!(matches!(err, CreateIsoError::RelativePath(_)));
    }
}
