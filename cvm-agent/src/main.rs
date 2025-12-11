use crate::{
    resources::{ApplicationMetadata, Resources},
    routes::{AppState, BootstrapContext, VmType, create_router},
};
use alloy::signers::k256::sha2::{Digest, Sha256};
use bollard::Docker;
use clap::{CommandFactory, Parser, error::ErrorKind};
use std::{
    fs::{self, File, create_dir_all},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::{TempDir, tempdir};
use tokio::{
    net::TcpListener,
    signal::{self, unix::SignalKind},
};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod heartbeat;
mod monitors;
mod resources;
mod routes;

#[derive(Parser)]
struct Cli {
    iso_mount_path: PathBuf,

    #[clap(long, default_value = default_version_path().into_os_string())]
    version_path: PathBuf,

    #[clap(long, default_value = default_vm_type_path().into_os_string())]
    vm_type_path: PathBuf,

    #[clap(long, default_value = default_log_file_path().into_os_string())]
    log_file: PathBuf,

    #[clap(long, default_value_t = default_bind_endpoint())]
    bind_endpoint: SocketAddr,
}

fn default_version_path() -> PathBuf {
    "/opt/nillion/nilcc-version".into()
}

fn default_vm_type_path() -> PathBuf {
    "/opt/nillion/nilcc-vm-type".into()
}

fn default_log_file_path() -> PathBuf {
    "/var/log/cvm-agent.log".into()
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
    let vm_type = fs::read_to_string(&cli.vm_type_path).expect("failed to read version");
    let vm_type = match vm_type.trim() {
        "cpu" => VmType::Cpu,
        "gpu" => VmType::Gpu,
        _ => panic!("unknown vm type {vm_type}"),
    };
    let state_dir = tempdir().expect("failed to create tempdir");
    println!("Writing state files to {}", state_dir.path().display());

    let resources = Resources::render(&metadata, &vm_type);
    let system_compose_path = state_dir.path().join("docker-compose.yaml");
    let caddy_path = state_dir.path().join("Caddyfile");
    let docker_config_path = state_dir.path().join("docker");
    fs::create_dir_all(&docker_config_path).expect("failed to create docker config path");
    fs::write(&system_compose_path, resources.docker_compose).expect("failed to write docker-compose.yaml");
    fs::write(&caddy_path, resources.caddyfile).expect("failed to write Caddyfile");

    let user_compose_path = cli.iso_mount_path.join("docker-compose.yaml");
    let user_docker_compose_sha256 = {
        let contents = fs::read(&user_compose_path).expect("failed to read user docker compose file");
        Sha256::digest(contents).into()
    };
    let external_files_path = cli.iso_mount_path.join("files");
    let context = BootstrapContext {
        system_docker_compose: system_compose_path,
        user_docker_compose: user_compose_path,
        user_docker_compose_sha256,
        external_files: external_files_path,
        caddy_config: caddy_path,
        docker_config: docker_config_path,
        version,
        vm_type,
        iso_mount: cli.iso_mount_path.clone(),
        event_holder: Default::default(),
        cpus: num_cpus::get() as u64,
        gpus: count_gpus() as u64,
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

fn count_gpus() -> usize {
    let mut id = 0;
    loop {
        let path = format!("/dev/nvidia{id}");
        if !fs::exists(&path).unwrap_or_default() {
            break;
        }
        id += 1;
    }
    id
}

fn setup_gpus(gpu_count: u64) {
    match gpu_count {
        0 => info!("No GPUs detected"),
        1 => {
            info!("Detected a single GPU, setting confidential compute ready state");
            std::process::Command::new("nvidia-smi")
                .args(["conf-compute", "-srs", "1"])
                .status()
                .expect("failed to run nvidia-smi");
        }
        _ => {
            info!("Detected {gpu_count} GPUs, setting multiple GPU mode");
            std::process::Command::new("nvidia-smi")
                .args(["conf-compute", "-mgm"])
                .status()
                .expect("failed to run nvidia-smi");
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Some(parent) = cli.log_file.parent() {
        create_dir_all(parent).expect("failed to create directory for log file");
    }
    let file = File::create(&cli.log_file).expect("Failed to create log file");
    let (writer, _guard) = tracing_appender::non_blocking(file);
    let file_layer = tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer);
    tracing_subscriber::registry()
        .with(file_layer)
        .with(
            tracing_subscriber::filter::EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let docker = Docker::connect_with_local_defaults().expect("failed to connect to docker daemon");
    let (_state_dir, context) = build_bootstrap_context(&cli);
    if matches!(context.vm_type, VmType::Gpu) {
        setup_gpus(context.gpus);
    }
    let state =
        Arc::new(AppState { docker, context, system_state: Default::default(), log_path: cli.log_file.clone() });
    let router = create_router(state.clone());
    let listener = TcpListener::bind(cli.bind_endpoint).await.expect("failed to bind");
    match axum::serve(listener, router).with_graceful_shutdown(shutdown_signal()).await {
        Ok(_) => info!("Shutting down"),
        Err(e) => error!("Failed to serve: {e}"),
    };
}
