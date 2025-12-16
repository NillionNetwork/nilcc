use anyhow::{Context, Result};
use axum_server::Handle;
use clap::{Args, Parser, Subcommand, ValueEnum};
use metrics_exporter_prometheus::PrometheusBuilder;
use nilcc_agent::{
    clients::{
        cvm_agent::DefaultCvmAgentClient,
        nilcc_api::{DummyNilccApiClient, HttpNilccApiClient, NilccApiClient, NilccApiClientArgs},
        qemu::{QemuClient, VmClient, VmDisplayMode},
    },
    config::{AgentConfig, AgentMode},
    heartbeat_verifier::VerifierKeys,
    repositories::sqlite::{RepositoryProvider, SqliteDb, SqliteRepositoryProvider},
    resources::SystemResources,
    routes::{AppState, Clients, Services, build_router},
    services::{
        disk::{
            ApplicationMetadata, ContainerMetadata, DefaultDiskService, DiskService, EnvironmentVariable, ExternalFile,
            IsoSpec,
        },
        proxy::{HaProxyProxyService, ProxyService, ProxyServiceArgs},
        upgrade::{DefaultUpgradeService, DefaultUpgradeServiceArgs},
        vm::{DefaultVmService, VmService, VmServiceArgs},
        workload::{DefaultWorkloadService, WorkloadService, WorkloadServiceArgs},
    },
    version,
    workers::{
        events::{EventWorker, EventWorkerArgs},
        heartbeat::{HeartbeatWorker, HeartbeatWorkerArgs},
    },
};
use nilcc_artifacts::{VmType, downloader::ArtifactsDownloader};
use rustls_acme::{AcmeConfig, AcmeState, caches::DirCache};
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
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

        /// Only print the qemu command being invoked instead of invoking it.
        #[clap(long, short)]
        print_only: bool,

        /// The identifier for the workload to debug.
        workload_id: Uuid,
    },

    /// Display system resources.
    Resources,

    /// Validate the config file.
    ValidateConfig {
        /// The path to the config file to validate.
        config: PathBuf,
    },

    /// Download resources.
    #[clap(subcommand)]
    Download(DownloadCommand),

    /// Generate a list of the available heartbeat verifier keys.
    VerifierKeys {
        /// Path to the agent configuration file
        #[clap(long, short)]
        config: PathBuf,
    },
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

#[derive(Subcommand)]
enum DownloadCommand {
    /// Download artifacts.
    Artifacts(DownloadArtifactsArgs),
}

#[derive(Args)]
struct DownloadArtifactsArgs {
    /// The base path where artifacts will be downloaded.
    download_path: PathBuf,

    /// The version to download
    version: String,

    #[clap(long, default_value_t = VmTypeArtifacts::All)]
    vm_type: VmTypeArtifacts,
}

#[derive(Clone, ValueEnum)]
enum VmTypeArtifacts {
    Cpu,
    Gpu,
    All,
}

impl fmt::Display for VmTypeArtifacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
            Self::All => write!(f, "all"),
        }
    }
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

fn load_config(config_path: &Path) -> Result<AgentConfig> {
    debug!("Loading configuration from: {config_path:?}");

    let config_file =
        fs::File::open(config_path).map_err(|e| anyhow::anyhow!("Failed to open config file {config_path:?}: {e}"))?;

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

async fn debug_workload(config: AgentConfig, workload_id: Uuid, print_only: bool) -> Result<()> {
    info!("Setting up dependencies");
    let nilcc_api_client: Arc<dyn NilccApiClient> = Arc::new(DummyNilccApiClient);

    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let repository_provider = Arc::new(SqliteRepositoryProvider::new(db.clone()));
    let workload = repository_provider.workloads(Default::default()).await?.find(workload_id).await?;
    let state_path = tempfile::tempdir().context("Failed to create tempdir")?;
    info!("Storing state in {}", state_path.path().display());

    let vm_client = Arc::new(QemuClient::new(config.qemu.system_bin.clone()));
    let cvm_agent_client = Arc::new(DefaultCvmAgentClient::new().context("Failed to create cvm-agent client")?);
    let event_sender = EventWorker::spawn(EventWorkerArgs {
        api_client: nilcc_api_client,
        repository_provider: repository_provider.clone(),
    });
    let vm_service = DefaultVmService::new(VmServiceArgs {
        vm_client: vm_client.clone(),
        cvm_agent_client: cvm_agent_client.clone(),
        state_path: state_path.path().into(),
        disk_service: Box::new(DefaultDiskService::new(config.qemu.img_bin)),
        cvm_artifacts_path: config.cvm.artifacts_path,
        zerossl_config: config.zerossl,
        docker_config: config.docker,
        event_sender,
        repository_provider: repository_provider.clone(),
        verifier_heartbeat_interval: config.verifier_heartbeat.interval_seconds,
        verifier_heartbeat_rpc: config.verifier_heartbeat.rpc_endpoint,
        verifier_contract_address: config.verifier_heartbeat.contract_address,
    })
    .await?;
    let mut spec = vm_service.create_workload_spec(&workload).await.context("Failed to create workload spec")?;
    spec.kernel_args.as_mut().expect("no kernel args").push_str(" console=ttyS0 debug_mode=1");
    spec.display = VmDisplayMode::Console;
    spec.port_forwarding.clear();

    let socket_path = state_path.path().join("qemu.sock");
    let args = vm_client.build_start_vm_args(&spec, &socket_path)?;
    if print_only {
        let args = args
            .into_iter()
            .map(|arg| if arg.contains(' ') { format!("'{arg}'") } else { arg })
            .collect::<Vec<_>>()
            .join(" ");
        println!("{} {args}", config.qemu.system_bin.display());
        return Ok(());
    }

    let mut child = std::process::Command::new(&config.qemu.system_bin)
        .args(args)
        .spawn()
        .context("Failed to start qemu-system")?;
    child.wait().context("Failed to wait for child process")?;
    Ok(())
}

async fn download_artifacts(args: DownloadArtifactsArgs) -> Result<()> {
    let DownloadArtifactsArgs { download_path, version, vm_type } = args;
    let vm_types = match vm_type {
        VmTypeArtifacts::Cpu => vec![VmType::Cpu],
        VmTypeArtifacts::Gpu => vec![VmType::Gpu],
        VmTypeArtifacts::All => vec![VmType::Cpu, VmType::Gpu],
    };
    let downloader = ArtifactsDownloader::new(version.clone(), vm_types);
    downloader.download(&download_path).await.context("Failed to download artifacts")?;
    Ok(())
}

fn validate_config(config_path: &Path) -> Result<()> {
    let config = fs::read(config_path).context("Failed to read config")?;
    serde_yaml::from_slice::<AgentConfig>(&config).context("Failed to deserialize config file")?;
    Ok(())
}

async fn run_daemon(config_path: PathBuf) -> Result<()> {
    let config = load_config(&config_path).context("Loading agent configuration")?;
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

    let vm_types = if system_resources.gpus.is_some() { vec![VmType::Cpu, VmType::Gpu] } else { vec![VmType::Cpu] };

    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let repository_provider = SqliteRepositoryProvider::new(db.clone());
    system_resources.adjust_gpu_assignment(&repository_provider).await.context("Failed to adjust GPU configs")?;

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

    let vm_client = Arc::new(QemuClient::new(config.qemu.system_bin));

    // We can't run more than one workload per CPU so use that as the upper bound
    let max_workloads = system_resources.cpus as usize;
    let verifier_keys = VerifierKeys::new(&config.verifier_heartbeat, max_workloads)?;

    let repository_provider = Arc::new(repository_provider);
    let cvm_agent_client = Arc::new(DefaultCvmAgentClient::new().context("Failed to create cvm-agent client")?);
    let event_sender = EventWorker::spawn(EventWorkerArgs {
        api_client: nilcc_api_client.clone(),
        repository_provider: repository_provider.clone(),
    });
    let vm_service = DefaultVmService::new(VmServiceArgs {
        vm_client,
        cvm_agent_client: cvm_agent_client.clone(),
        state_path: config.vm_store,
        disk_service: Box::new(DefaultDiskService::new(config.qemu.img_bin)),
        cvm_artifacts_path: config.cvm.artifacts_path.clone(),
        zerossl_config: config.zerossl,
        docker_config: config.docker,
        event_sender,
        repository_provider: repository_provider.clone(),
        verifier_heartbeat_interval: config.verifier_heartbeat.interval_seconds,
        verifier_heartbeat_rpc: config.verifier_heartbeat.rpc_endpoint,
        verifier_contract_address: config.verifier_heartbeat.contract_address,
    })
    .await?;
    let workload_service = DefaultWorkloadService::new(WorkloadServiceArgs {
        vm_service: Box::new(vm_service),
        repository_provider: repository_provider.clone(),
        resources: system_resources.clone(),
        open_ports: config.sni_proxy.start_port_range..config.sni_proxy.end_port_range,
        proxy_service: Box::new(proxy_service),
        verifier_keys: verifier_keys.clone(),
    })
    .await
    .context("Creating workload service")?;
    info!("Bootstrapping existing workloads");
    workload_service.bootstrap().await?;

    let workload_service = Arc::new(workload_service);
    let upgrade_service = Arc::new(DefaultUpgradeService::new(DefaultUpgradeServiceArgs {
        repository_provider: repository_provider.clone(),
        config_file_path: config_path,
        cvm_artifacts_path: config.cvm.artifacts_path.clone(),
        vm_types,
    }));
    let state = AppState {
        services: Services { workload: workload_service.clone(), upgrade: upgrade_service.clone() },
        clients: Clients { cvm_agent: cvm_agent_client },
        resource_limits: config.resources.limits,
        agent_domain: config.api.domain.clone(),
        verifier_keys,
    };
    let router = build_router(state, config.api.token);
    let handle = Handle::new();
    tokio::spawn(shutdown_handler(handle.clone()));

    info!("Starting heartbeat worker");

    HeartbeatWorker::spawn(HeartbeatWorkerArgs {
        api_client: nilcc_api_client,
        provider: repository_provider.clone(),
        upgrader: upgrade_service,
    });

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

async fn print_verifier_keys(config: AgentConfig) -> Result<()> {
    let resources = SystemResources::gather(config.resources.reserved).await?;
    let total = resources.cpus as usize;
    let keys = VerifierKeys::new(&config.verifier_heartbeat, total)?;
    let mut generated = Vec::new();
    for _ in 0..total {
        let key = keys.next_key()?;
        generated.push(key);
    }
    for key in generated {
        let private = hex::encode(key.secret_key());
        let public = hex::encode(key.public_key());
        println!("- private key {private} (public key {public})");
    }
    Ok(())
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
            run_daemon(config).await?;
            Ok(())
        }
        Command::Debug { config, workload_id, print_only } => {
            let agent_config = load_config(&config).context("Loading agent configuration")?;
            debug_workload(agent_config, workload_id, print_only).await?;
            Ok(())
        }
        Command::Resources => {
            let resources = SystemResources::gather(Default::default()).await?;
            let resources = serde_json::to_string_pretty(&resources).expect("failed to serialize");
            println!("{resources}");
            Ok(())
        }
        Command::VerifierKeys { config } => {
            let agent_config = load_config(&config).context("Loading agent configuration")?;
            print_verifier_keys(agent_config).await?;
            Ok(())
        }
        Command::ValidateConfig { config } => {
            validate_config(&config).context("Invalid config file")?;
            println!("Config file is valid");
            Ok(())
        }
        Command::Download(DownloadCommand::Artifacts(args)) => download_artifacts(args).await,
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
