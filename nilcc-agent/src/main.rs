use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use metrics_exporter_prometheus::PrometheusBuilder;
use nilcc_agent::{
    clients::{
        nilcc_api::{DummyNilccApiClient, HttpNilccApiClient, NilccApiClient, NilccApiClientArgs},
        qemu::QemuClient,
    },
    config::{AgentConfig, AgentMode},
    repositories::{
        sqlite::SqliteDb,
        workload::{SqliteWorkloadRepository, WorkloadRepository},
    },
    resources::SystemResources,
    routes::{build_router, AppState, Services},
    services::{
        disk::{ApplicationMetadata, ContainerMetadata, DefaultDiskService, DiskService, EnvironmentVariable, IsoSpec},
        proxy::HaProxyProxyService,
        vm::{DefaultVmService, VmServiceArgs},
        workload::{DefaultWorkloadService, WorkloadServiceArgs},
    },
    version,
};
use std::{fs, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, signal};
use tracing::{debug, info, level_filters::LevelFilter};

#[derive(Parser)]
#[clap(author, version = version::agent_version(), about = "nilcc agent")]
struct Cli {
    /// The command to be ran.
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// ISO commands.
    #[clap(subcommand)]
    Iso(IsoCommand),

    /// Run the agent in daemon mode and connect to the nilcc API.
    Daemon {
        /// Path to the agent configuration file
        #[clap(long, short)]
        config: PathBuf,
    },

    /// Display system resources.
    Resources,
}

#[derive(Subcommand)]
enum IsoCommand {
    /// Create an ISO for an application to be run inside a VM.
    Create {
        /// The container that is the entry point to the application.
        #[clap(short, long)]
        container: String,

        /// The port where the entry point container is reached.
        #[clap(short, long)]
        port: u16,

        /// The hostname to use for the generated TLS certificate.
        #[clap(short = 'H', long)]
        hostname: String,

        /// The path in which to write the output ISO file.
        #[clap(short, long)]
        output: PathBuf,

        /// An environment variable that will be set when the docker compose is ran.
        #[clap(short, long = "env")]
        environment_variables: Vec<EnvironmentVariable>,

        /// The path to the docker compose to be ran.
        docker_compose_path: PathBuf,
    },
}

async fn run_iso_command(command: IsoCommand) -> Result<()> {
    match command {
        IsoCommand::Create { container, port, hostname, output, docker_compose_path, environment_variables } => {
            let compose = std::fs::read_to_string(docker_compose_path).context("reading docker compose")?;
            let spec = IsoSpec {
                docker_compose_yaml: compose,
                metadata: ApplicationMetadata { hostname, api: ContainerMetadata { container, port } },
                environment_variables,
            };
            let disk_service = DefaultDiskService::new("qemu-img".into());
            disk_service.create_application_iso(&output, spec).await.context("creating ISO")?;
            Ok(())
        }
    }
}

fn load_config(config_path: PathBuf) -> Result<AgentConfig> {
    debug!("Loading configuration from: {config_path:?}");

    let config_file =
        fs::File::open(&config_path).map_err(|e| anyhow::anyhow!("Failed to open config file {config_path:?}: {e}"))?;

    let config: AgentConfig = serde_yaml::from_reader(config_file)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML from config file {config_path:?}: {e}"))?;

    Ok(config)
}

async fn run_daemon(config: AgentConfig) -> Result<()> {
    info!("Setting up dependencies");
    let nilcc_api_client: Arc<dyn NilccApiClient> = match config.controller {
        AgentMode::Standalone => Arc::new(DummyNilccApiClient),
        AgentMode::Remote(remote) => Arc::new(HttpNilccApiClient::new(NilccApiClientArgs {
            api_base_url: remote.endpoint,
            api_key: remote.key,
            agent_id: config.agent_id,
        })?),
    };

    PrometheusBuilder::default()
        .with_http_listener(config.metrics.bind_endpoint)
        .install()
        .context("Failed to start metrics exporter")?;

    let system_resources =
        SystemResources::gather(config.resources.reserved).await.context("Failed to find resources")?;
    system_resources.create_gpu_vfio_devices().await.context("Failed to create PCI VFIO GPU devices")?;

    info!("Registering with API");
    nilcc_api_client.register(&system_resources).await.context("Failed to register")?;

    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let workload_repository = Arc::new(SqliteWorkloadRepository::new(db.clone()));
    let existing_workloads = workload_repository.list().await.context("Failed to find existing workloads")?;
    let proxied_vms = existing_workloads.iter().map(Into::into).collect();
    let proxy_service = HaProxyProxyService::new(
        config.sni_proxy.config_file_path,
        config.sni_proxy.master_socket_path,
        config.sni_proxy.timeouts,
        config.sni_proxy.dns_subdomain,
        config.sni_proxy.max_connections,
        proxied_vms,
    );

    let vm_client = Arc::new(QemuClient::new(config.qemu.system_bin));
    let vm_service = DefaultVmService::new(VmServiceArgs {
        vm_client,
        nilcc_api_client,
        state_path: config.vm_store,
        disk_service: Box::new(DefaultDiskService::new(config.qemu.img_bin)),
        cvm_config: config.cvm,
    })
    .await?;
    let workload_service = DefaultWorkloadService::new(WorkloadServiceArgs {
        vm_service: Box::new(vm_service),
        repository: Box::new(SqliteWorkloadRepository::new(db)),
        resources: system_resources.clone(),
        open_ports: config.sni_proxy.start_port_range..config.sni_proxy.end_port_range,
        proxy_service: Box::new(proxy_service),
    })
    .await
    .context("Creating workload service")?;
    let state = AppState {
        services: Services { workload: Arc::new(workload_service) },
        resource_limits: config.resources.limits,
    };
    let router = build_router(state);
    let listener = TcpListener::bind(config.api.bind_endpoint).await.context("Failed to bind")?;
    let server = axum::serve(listener, router).with_graceful_shutdown(shutdown_signal());
    info!("Listening to requests on {}", config.api.bind_endpoint);

    server.await.context("Failed to serve")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Received shutdown signal");
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli { command } = cli;
    match command {
        Command::Iso(command) => run_iso_command(command).await,
        Command::Daemon { config } => {
            let agent_config = load_config(config).context("Loading agent configuration")?;
            run_daemon(agent_config).await?;
            Ok(())
        }
        Command::Resources => {
            let resources = SystemResources::gather(Default::default()).await?;
            let resources = serde_json::to_string_pretty(&resources).expect("failed to serialize");
            println!("{resources}");
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::filter::EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("Error running CLI: {e:#}");
        std::process::exit(1);
    }

    Ok(())
}
