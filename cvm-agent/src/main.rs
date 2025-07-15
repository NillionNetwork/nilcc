use crate::{
    resources::{ApplicationMetadata, Resources},
    routes::create_router,
};
use bollard::Docker;
use clap::{error::ErrorKind, CommandFactory, Parser};
use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::{tempdir, TempDir};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    signal::{self, unix::SignalKind},
};
use tracing::{error, info};

const COMPOSE_PROJECT_NAME: &str = "cvm";

mod resources;
mod routes;

#[derive(Parser)]
struct Cli {
    iso_mount_path: PathBuf,

    #[clap(long, default_value = default_version_path().into_os_string())]
    version_path: PathBuf,

    #[clap(long, default_value = default_vm_type_path().into_os_string())]
    vm_type_path: PathBuf,

    #[clap(long, default_value_t = default_bind_endpoint())]
    bind_endpoint: SocketAddr,
}

fn default_version_path() -> PathBuf {
    "/opt/nillion/nilcc-version".into()
}

fn default_vm_type_path() -> PathBuf {
    "/opt/nillion/nilcc-vm-type".into()
}

fn default_bind_endpoint() -> SocketAddr {
    SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 59666).into()
}

fn load_metadata(path: &Path) -> Result<ApplicationMetadata, Box<dyn std::error::Error>> {
    let metadata = fs::read_to_string(path)?;
    let metadata = serde_json::from_str(&metadata)?;
    Ok(metadata)
}

fn launch_docker_compose(cli: &Cli) -> (TempDir, Child) {
    let metadata = match load_metadata(&cli.iso_mount_path.join("metadata.json")) {
        Ok(metadata) => metadata,
        Err(e) => {
            Cli::command().error(ErrorKind::InvalidValue, format!("could not load metadata: {e}")).exit();
        }
    };
    let version = fs::read_to_string(&cli.version_path).expect("failed to read version").trim().to_string();
    let vm_type = fs::read_to_string(&cli.vm_type_path).expect("failed to read version").trim().to_string();
    let state_dir = tempdir().expect("failed to create tempdir");
    println!("Writing state files to {}", state_dir.path().display());

    let resources = Resources::render(&metadata);
    let our_compose_path = state_dir.path().join("docker-compose.yaml");
    let our_caddy_path = state_dir.path().join("Caddyfile");
    fs::write(&our_compose_path, resources.docker_compose).expect("failed to write docker-compose.yaml");
    fs::write(&our_caddy_path, resources.caddyfile).expect("failed to write Caddyfile");

    let iso_compose_path = cli.iso_mount_path.join("docker-compose.yaml");
    let external_files_path = cli.iso_mount_path.join("files");
    let mut command = Command::new("docker");
    let command = command
        .current_dir(&cli.iso_mount_path)
        // pass in `FILES` which points to `<iso>/files`
        .env("FILES", external_files_path.into_os_string())
        // pass in other env vars that are needed by our compose file
        .env("CADDY_INPUT_FILE", our_caddy_path.into_os_string())
        .env("NILCC_VERSION", version)
        .env("NILCC_VM_TYPE", vm_type)
        .arg("compose")
        // set a well defined project name, this is used as a prefix for container names
        .arg("-p")
        .arg(COMPOSE_PROJECT_NAME)
        // point to the user provided compose file first
        .arg("-f")
        .arg(iso_compose_path)
        // the outs
        .arg("-f")
        .arg(our_compose_path)
        .arg("up");
    let child = command.spawn().expect("failed to spawn docker");
    (state_dir, child)
}

async fn shutdown_signal(mut docker_compose: Child) {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install ctrl-c handler");
    };

    let terminate = async {
        signal::unix::signal(SignalKind::terminate()).expect("failed to install signal handler").recv().await;
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("Received ctrl-c");
        },
        _ = terminate => {
            info!("Received SIGTERM");
        },
        _ = docker_compose.wait() => {
            info!("Docker compose child terminated")
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();
    let docker = Arc::new(Docker::connect_with_local_defaults().expect("failed to connect to docker daemon"));
    let (_state_dir, compose_child) = launch_docker_compose(&cli);
    let router = create_router(docker);
    let listener = TcpListener::bind(cli.bind_endpoint).await.expect("failed to bind");
    if let Err(e) = axum::serve(listener, router).with_graceful_shutdown(shutdown_signal(compose_child)).await {
        error!("Failed to serve: {e}");
    }
}
