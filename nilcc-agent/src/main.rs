use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use metrics_exporter_prometheus::PrometheusBuilder;
use nilcc_agent::{
    agent_service::{AgentService, AgentServiceArgs},
    build_info,
    config::{AgentConfig, ApiConfig},
    http_client::RestNilccApiClient,
    iso::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoMaker, IsoMetadata, IsoSpec},
    output::{serialize_error, serialize_output, SerializeAsAny},
    qemu_client::QemuClient,
    repositories::{sqlite::SqliteDb, workload::SqliteWorkloadRepository},
    resources::SystemResources,
    services::{
        disk::DefaultDiskService,
        sni_proxy::HaProxySniProxyService,
        vm::{DefaultVmService, VmServiceArgs},
    },
};
use serde::Serialize;
use std::{fs, ops::Deref, path::PathBuf, sync::Arc};
use tracing::debug;

#[derive(Parser)]
#[clap(author, version = build_info::get_agent_version(), about = "nilCC Agent CLI")]
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

    /// Run the agent in daemon mode and connect to nilCC API.
    Daemon {
        /// Path to the agent configuration file
        #[clap(long, short, default_value = "nilcc-agent-config.yaml")]
        config: PathBuf,
    },

    /// Show metal instance information
    Info,
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

/// JSON wrapper for success responses
#[derive(Serialize)]
struct ActionOutput<T: Serialize> {
    status: String,
    details: T,
}

async fn run_iso_command(command: IsoCommand) -> Result<ActionOutput<IsoMetadata>> {
    match command {
        IsoCommand::Create { container, port, hostname, output, docker_compose_path, environment_variables } => {
            let compose = std::fs::read_to_string(docker_compose_path).context("reading docker compose")?;
            let spec = IsoSpec {
                docker_compose_yaml: compose,
                metadata: ApplicationMetadata { hostname, api: ContainerMetadata { container, port } },
                environment_variables,
            };
            let details = IsoMaker.create_application_iso(&output, spec).await.context("creating ISO")?;
            Ok(ActionOutput { status: "created".into(), details })
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
    let ApiConfig { endpoint, key, sync_interval } = config.api;

    PrometheusBuilder::default()
        .with_http_listener(config.metrics.bind_endpoint)
        .install()
        .context("Failed to start metrics exporter")?;

    let metal_details = SystemResources::gather(config.resources.reserved).await.context("Failed to find resources")?;
    let api_client = Box::new(RestNilccApiClient::new(endpoint, key)?);
    let db = SqliteDb::connect(&config.db.url).await.context("Failed to create database")?;
    let workload_repository = Arc::new(SqliteWorkloadRepository::new(db));
    let disk_service = DefaultDiskService::new(config.qemu.img_bin);
    let qemu_client = QemuClient::new(config.qemu.system_bin);
    let sni_proxy_service = Box::new(HaProxySniProxyService::new(
        config.sni_proxy.config_file_path,
        config.sni_proxy.ha_proxy_config_reload_command,
        config.sni_proxy.timeouts,
        config.sni_proxy.dns_subdomain,
        config.sni_proxy.max_connections,
    ));
    let vm_service = DefaultVmService::new(VmServiceArgs {
        state_path: config.vm_store,
        vm_client: Box::new(qemu_client),
        disk_service: Box::new(disk_service),
        workload_repository: workload_repository.clone(),
        cvm_config: config.cvm,
        sni_proxy_service,
    })
    .await
    .context("Failed to create vm service")?;

    let args = AgentServiceArgs {
        agent_id: config.agent_id,
        api_client,
        vm_service: Box::new(vm_service),
        workload_repository,
        sync_interval,
        start_port_range: config.sni_proxy.start_port_range,
        end_port_range: config.sni_proxy.end_port_range,
        metal_details,
    };
    let agent_service = AgentService::new(args);

    let _handle = agent_service.run().await.context("AgentService failed to start and register")?;
    debug!("AgentService is running.");

    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn run(cli: Cli) -> anyhow::Result<Box<dyn SerializeAsAny>> {
    let Cli { command } = cli;
    match command {
        Command::Iso(command) => Ok(Box::new(run_iso_command(command).await?)),
        Command::Daemon { config } => {
            let agent_config = load_config(config).context("Loading agent configuration")?;
            run_daemon(agent_config).await?;
            Ok(Box::new(()))
        }
        Command::Info => {
            let instance_details = SystemResources::gather(Default::default()).await?;
            Ok(Box::new(instance_details))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let cli = Cli::parse();
    match run(cli).await {
        Ok(val) => println!("{}", serialize_output(val.deref()).unwrap()),
        Err(e) => {
            eprintln!("{}", serialize_error(&e));
            std::process::exit(1);
        }
    }

    Ok(())
}
