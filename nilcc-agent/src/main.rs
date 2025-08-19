use anyhow::{Context, Result};
use axum_server::Handle;
use clap::{Parser, Subcommand};
use metrics_exporter_prometheus::PrometheusBuilder;
use nilcc_agent::{
    clients::{
        cvm_agent::DefaultCvmAgentClient,
        nilcc_api::{DummyNilccApiClient, HttpNilccApiClient, NilccApiClient, NilccApiClientArgs},
        qemu::{QemuClient, VmClient, VmDisplayMode},
    },
    config::{AgentConfig, AgentMode},
    repositories::sqlite::{RepositoryProvider, SqliteDb, SqliteRepositoryProvider},
    resources::SystemResources,
    routes::{build_router, AppState, Clients, Services},
    services::{
        disk::{
            ApplicationMetadata, ContainerMetadata, DefaultDiskService, DiskService, EnvironmentVariable, ExternalFile,
            IsoSpec,
        },
        proxy::{HaProxyProxyService, ProxyService, ProxyServiceArgs},
        vm::{DefaultVmService, VmService, VmServiceArgs},
        workload::{DefaultWorkloadService, WorkloadService, WorkloadServiceArgs},
    },
    version,
    workers::heartbeat::HeartbeatWorker,
};
use rustls_acme::{caches::DirCache, AcmeConfig, AcmeState};
use std::{fs, io, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use tokio::signal;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, level_filters::LevelFilter, warn};
use uuid::Uuid;

const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(10);

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

    /// Run the agent in daemon mode and connect to the nilcc API.
    Debug {
        /// Path to the agent configuration file
        #[clap(long, short)]
        config: PathBuf,

        /// The identifier for the workload to debug.
        workload_id: Uuid,
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
        environment_variables: Vec<CliEnvironmentVariable>,

        /// The external files to include in the ISO.
        #[clap(short = 'f', long = "file")]
        files: Vec<CliExternalFile>,

        /// The path to the docker compose to be ran.
        docker_compose_path: PathBuf,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct CliExternalFile(ExternalFile);

impl FromStr for CliExternalFile {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, path) = s.split_once('=').context("expected environment variable in <name>=<value> syntax")?;
        let name = name.trim().to_string();
        let contents = std::fs::read(path).context("Failed to read external file")?;
        Ok(Self(ExternalFile { name, contents }))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CliEnvironmentVariable(EnvironmentVariable);

impl FromStr for CliEnvironmentVariable {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, value) = s.split_once('=').context("expected environment variable in <name>=<value> syntax")?;
        let name = name.trim().to_string();
        let value = value.to_string();
        Ok(Self(EnvironmentVariable { name, value }))
    }
}

async fn run_iso_command(command: IsoCommand) -> Result<()> {
    match command {
        IsoCommand::Create { container, port, hostname, output, docker_compose_path, environment_variables, files } => {
            let compose = std::fs::read_to_string(docker_compose_path).context("reading docker compose")?;
            let spec = IsoSpec {
                docker_compose_yaml: compose,
                metadata: ApplicationMetadata { hostname, api: ContainerMetadata { container, port } },
                environment_variables: environment_variables.into_iter().map(|e| e.0).collect(),
                files: files.into_iter().map(|f| f.0).collect(),
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

async fn process_acme_events(mut state: AcmeState<io::Error, io::Error>) {
    while let Some(event) = state.next().await {
        match event {
            Ok(event) => info!("ACME event: {event:?}"),
            Err(e) => error!("ACME error: {e:?}"),
        }
    }
    warn!("Reached end of ACME event stream")
}

async fn debug_workload(config: AgentConfig, workload_id: Uuid) -> Result<()> {
    info!("Setting up dependencies");
    let nilcc_api_client: Arc<dyn NilccApiClient> = Arc::new(DummyNilccApiClient);

    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let repository_provider = SqliteRepositoryProvider::new(db.clone());
    let workload = repository_provider.workloads(Default::default()).await?.find(workload_id).await?;
    let state_path = tempfile::tempdir().context("Failed to create tempdir")?;
    info!("Storing state in {}", state_path.path().display());

    let vm_client = Arc::new(QemuClient::new(config.qemu.system_bin.clone()));
    let cvm_agent_client = Arc::new(DefaultCvmAgentClient::new().context("Failed to create cvm-agent client")?);
    let vm_service = DefaultVmService::new(VmServiceArgs {
        vm_client: vm_client.clone(),
        nilcc_api_client,
        cvm_agent_client: cvm_agent_client.clone(),
        state_path: state_path.path().into(),
        disk_service: Box::new(DefaultDiskService::new(config.qemu.img_bin)),
        cvm_config: config.cvm,
        zerossl_config: config.zerossl,
        docker_config: config.docker,
    })
    .await?;
    let mut spec = vm_service.create_workload_spec(&workload).await.context("Failed to create workload spec")?;
    spec.kernel_args.as_mut().expect("no kernel args").push_str(" console=ttyS0");
    spec.display = VmDisplayMode::Console;
    spec.port_forwarding.clear();

    let socket_path = state_path.path().join("qemu.sock");
    let args = vm_client.build_start_vm_args(&spec, &socket_path)?;
    let mut child = std::process::Command::new(&config.qemu.system_bin)
        .args(args)
        .spawn()
        .context("Failed to start qemu-system")?;
    child.wait().context("Failed to wait for child process")?;
    Ok(())
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

    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let repository_provider = SqliteRepositoryProvider::new(db.clone());
    let proxied_vms = {
        let mut workload_repository = repository_provider.workloads(Default::default()).await?;
        let existing_workloads = workload_repository.list().await.context("Failed to find existing workloads")?;
        existing_workloads.iter().map(Into::into).collect()
    };
    let proxy_service = HaProxyProxyService::new(ProxyServiceArgs {
        config_file_path: config.sni_proxy.config_file_path.clone(),
        master_socket_path: config.sni_proxy.master_socket_path,
        timeouts: config.sni_proxy.timeouts,
        agent_domain: config.api.domain.clone(),
        agent_port: config.api.bind_endpoint.port(),
        max_connections: config.sni_proxy.max_connections,
        proxied_vms,
        reload_config: config.sni_proxy.reload_config,
    });
    info!("Storing current proxy config into {}", config.sni_proxy.config_file_path.display());
    proxy_service.persist_current_config().await.context("Failed to store current proxy config")?;

    info!("Finding public IPv4 address");
    let public_ip = SystemResources::find_public_ip().context("Failed to find public IPv4 address")?;

    info!("Registering with API");
    nilcc_api_client.register(&config.api, &system_resources, public_ip).await.context("Failed to register")?;

    info!("Starting heartbeat worker");
    HeartbeatWorker::spawn(nilcc_api_client.clone());

    let vm_client = Arc::new(QemuClient::new(config.qemu.system_bin));
    let cvm_agent_client = Arc::new(DefaultCvmAgentClient::new().context("Failed to create cvm-agent client")?);
    let vm_service = DefaultVmService::new(VmServiceArgs {
        vm_client,
        nilcc_api_client,
        cvm_agent_client: cvm_agent_client.clone(),
        state_path: config.vm_store,
        disk_service: Box::new(DefaultDiskService::new(config.qemu.img_bin)),
        cvm_config: config.cvm,
        zerossl_config: config.zerossl,
        docker_config: config.docker,
    })
    .await?;
    let workload_service = DefaultWorkloadService::new(WorkloadServiceArgs {
        vm_service: Box::new(vm_service),
        repository_provider: Box::new(repository_provider),
        resources: system_resources.clone(),
        open_ports: config.sni_proxy.start_port_range..config.sni_proxy.end_port_range,
        proxy_service: Box::new(proxy_service),
    })
    .await
    .context("Creating workload service")?;
    info!("Bootstrapping existing workloads");
    workload_service.bootstrap().await?;
    let workload_service = Arc::new(workload_service);
    let state = AppState {
        services: Services { workload: workload_service.clone() },
        clients: Clients { cvm_agent: cvm_agent_client },
        resource_limits: config.resources.limits,
        agent_domain: config.api.domain.clone(),
    };
    let router = build_router(state, config.api.token);
    let handle = Handle::new();
    tokio::spawn(shutdown_handler(handle.clone()));

    info!("Listening to requests on {}", config.api.bind_endpoint);
    let server = axum_server::bind(config.api.bind_endpoint).handle(handle);
    let result = match config.tls {
        Some(tls) => {
            info!(
                "Setting up TLS certificate generation, using domain = {}, cert cache = {}, ACME contact = {}",
                config.api.domain,
                tls.cert_cache.display(),
                tls.acme_contact
            );
            let state = AcmeConfig::new([config.api.domain])
                .contact([format!("mailto:{}", tls.acme_contact)])
                .cache(DirCache::new(tls.cert_cache.clone()))
                .directory_lets_encrypt(true)
                .state();
            let acceptor = state.axum_acceptor(state.default_rustls_config());
            // Spin up a task that polls the ACME cert generation future
            tokio::spawn(process_acme_events(state));
            server.acceptor(acceptor).serve(router.into_make_service()).await
        }
        None => server.serve(router.into_make_service()).await,
    };
    result.context("Failed to serve")
}

async fn shutdown_handler(handle: Handle) {
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
    handle.graceful_shutdown(Some(SHUTDOWN_GRACE_PERIOD));
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
        Command::Debug { config, workload_id } => {
            let agent_config = load_config(config).context("Loading agent configuration")?;
            debug_workload(agent_config, workload_id).await?;
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
