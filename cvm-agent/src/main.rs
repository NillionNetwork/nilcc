use crate::{
    resources::{ApplicationMetadata, Resources},
    routes::{create_router, AppState, BootstrapContext, SystemState},
};
use bollard::Docker;
use clap::{error::ErrorKind, CommandFactory, Parser};
use std::{
    fs, mem,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::{tempdir, TempDir};
use tokio::{
    net::TcpListener,
    signal::{self, unix::SignalKind},
};
use tracing::{error, info};

mod caddy;
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

fn build_bootstrap_context(cli: &Cli) -> (TempDir, BootstrapContext) {
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
    let system_compose_path = state_dir.path().join("docker-compose.yaml");
    let caddy_path = state_dir.path().join("caddy.json");
    fs::write(&system_compose_path, resources.docker_compose).expect("failed to write docker-compose.yaml");
    fs::write(&caddy_path, resources.caddyfile).expect("failed to write Caddyfile");

    let user_compose_path = cli.iso_mount_path.join("docker-compose.yaml");
    let external_files_path = cli.iso_mount_path.join("files");
    let context = BootstrapContext {
        system_docker_compose: system_compose_path,
        user_docker_compose: user_compose_path,
        external_files: external_files_path,
        caddy_config: caddy_path,
        version,
        vm_type,
        iso_mount: cli.iso_mount_path.clone(),
    };
    (state_dir, context)
}

async fn shutdown_signal() {
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
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();
    let docker = Docker::connect_with_local_defaults().expect("failed to connect to docker daemon");
    let (_state_dir, context) = build_bootstrap_context(&cli);
    let state = Arc::new(AppState { docker, context, system_state: Default::default() });
    let router = create_router(state.clone());
    let listener = TcpListener::bind(cli.bind_endpoint).await.expect("failed to bind");
    if let Err(e) = axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await {
        error!("Failed to serve: {e}");
    }
    info!("Shutting down");
    let system_state = {
        let mut state = state.system_state.lock().unwrap();
        mem::take(&mut *state)
    };
    match system_state {
        SystemState::WaitingBootstrap => {
            info!("Docker compose is not running");
        }
        SystemState::Starting(mut child) | SystemState::Ready(mut child) => {
            info!("Shutting docker compose down");
            child.kill().await.expect("failed to kill child");
        }
    };
}
