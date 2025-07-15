use crate::resources::{ApplicationMetadata, Resources};
use clap::{error::ErrorKind, CommandFactory, Parser};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::tempdir;
mod resources;

#[derive(Parser)]
struct Cli {
    iso_mount_path: PathBuf,

    #[clap(long, default_value = default_version_path().into_os_string())]
    version_path: PathBuf,

    #[clap(long, default_value = default_vm_type_path().into_os_string())]
    vm_type_path: PathBuf,
}

fn default_version_path() -> PathBuf {
    "/opt/nillion/nilcc-version".into()
}

fn default_vm_type_path() -> PathBuf {
    "/opt/nillion/nilcc-vm-type".into()
}

fn load_metadata(path: &Path) -> Result<ApplicationMetadata, Box<dyn std::error::Error>> {
    let metadata = fs::read_to_string(path)?;
    let metadata = serde_json::from_str(&metadata)?;
    Ok(metadata)
}

fn main() {
    let cli = Cli::parse();
    let metadata = match load_metadata(&cli.iso_mount_path.join("metadata.json")) {
        Ok(metadata) => metadata,
        Err(e) => {
            Cli::command().error(ErrorKind::InvalidValue, format!("could not load metadata: {e}")).exit();
        }
    };
    let version = fs::read_to_string(cli.version_path).expect("failed to read version").trim().to_string();
    let vm_type = fs::read_to_string(cli.vm_type_path).expect("failed to read version").trim().to_string();
    let state_path = tempdir().expect("failed to create tempdir");
    println!("Writing state files to {}", state_path.path().display());

    let resources = Resources::render(&metadata);
    let our_compose_path = state_path.path().join("docker-compose.yaml");
    let our_caddy_path = state_path.path().join("Caddyfile");
    fs::write(&our_compose_path, resources.docker_compose).expect("failed to write docker-compose.yaml");
    fs::write(&our_caddy_path, resources.caddyfile).expect("failed to write Caddyfile");

    let iso_compose_path = cli.iso_mount_path.join("docker-compose.yaml");
    let external_files_path = cli.iso_mount_path.join("files");
    let mut command = Command::new("docker");
    let command = command
        .current_dir(&cli.iso_mount_path)
        .env("FILES", external_files_path.into_os_string())
        .env("CADDY_INPUT_FILE", our_caddy_path.into_os_string())
        .env("NILCC_VERSION", version)
        .env("NILCC_VM_TYPE", vm_type)
        .arg("compose")
        .arg("-f")
        .arg(iso_compose_path)
        .arg("-f")
        .arg(our_compose_path)
        .arg("up");
    let mut handle = command.spawn().expect("failed to spawn docker");
    handle.wait().expect("error while waiting for docker");
}
