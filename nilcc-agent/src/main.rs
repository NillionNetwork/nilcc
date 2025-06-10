use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use nilcc_agent::{
    agent_service::{AgentService, AgentServiceArgs},
    build_info,
    config::{AgentConfig, ApiConfig},
    http_client::RestNilccApiClient,
    iso::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoMaker, IsoMetadata, IsoSpec},
    output::{serialize_error, serialize_output, SerializeAsAny},
    qemu_client::{HardDiskFormat, HardDiskSpec, QemuClient, QemuClientError, VmClient, VmDetails, VmSpec},
};
use serde::Serialize;
use std::{env, fs, ops::Deref, path::PathBuf};
use tracing::debug;
use users::{get_current_uid, get_user_by_uid, os::unix::UserExt};
use uuid::Uuid;

const DEFAULT_QEMU_SYSTEM: &str = "qemu-system-x86_64";
const DEFAULT_QEMU_IMG: &str = "qemu-img";
const DEFAULT_VM_STORE: &str = ".nilcc/vms";

#[derive(Parser)]
#[clap(author, version = build_info::get_agent_version(), about = "nilCC Agent CLI")]
struct Cli {
    /// The command to be ran.
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// VM commands.
    Vm {
        #[clap(flatten)]
        args: VmCommandArgs,
    },

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

#[derive(Args)]
struct VmCommandArgs {
    /// Directory where VM folders live (default: $HOME/.nilcc/vms)
    #[clap(env, long, short, default_value_os_t = default_vm_store())]
    vm_store: PathBuf,

    /// Optional qemu-system binary
    #[clap(
        long = "qemu-system-bin",
        env = "VM_QEMU_SYSTEM_BIN",
        default_value = DEFAULT_QEMU_SYSTEM
    )]
    qemu_system_bin: PathBuf,

    /// Optional; qemu-img binary
    #[clap(
        long = "qemu-img-bin",
        env = "VM_QEMU_IMG_BIN",
        default_value = DEFAULT_QEMU_IMG
    )]
    qemu_img_bin: PathBuf,

    /// The command to be ran.
    #[clap(subcommand)]
    command: VmCommand,
}

#[derive(Subcommand)]
enum VmCommand {
    /// Create and start a fresh VM
    Create {
        /// VM identifier.
        id: Uuid,

        /// vCPUs
        #[clap(long)]
        cpu: u8,

        /// RAM in MiB
        #[clap(long)]
        ram_mib: u32,

        /// Disk size in GiB
        #[clap(long)]
        disk_gib: u32,

        /// Optional ISO to attach as CD-ROM
        #[clap(long = "cdrom-path")]
        cdrom_iso_path: Option<PathBuf>,

        /// Enable NVIDIA GPU passthrough
        #[clap(long)]
        gpu: bool,

        /// Port-forwarding rule(s) HOST:VM (may be repeated)
        #[clap(long = "portfwd")]
        port_forward: Vec<String>,

        /// Optional path to a BIOS file
        #[clap(long)]
        bios_path: Option<PathBuf>,

        /// Optional path to a kernel file
        #[clap(long)]
        kernel_path: Option<PathBuf>,

        /// Optional path to a initrd file
        #[clap(long)]
        initrd_path: Option<PathBuf>,

        /// Optional kernel parameters
        #[clap(long)]
        kernel_args: Option<String>,

        /// Show GTK ;display for VM, defaults to headless VM
        #[clap(long)]
        display_gtk: bool,

        /// Enable CVM
        #[clap(long)]
        enable_cvm: bool,
    },

    /// Start already created VM
    Start { id: Uuid },

    /// Gracefully stop running VM or force stop if `force` is true
    Stop {
        id: Uuid,
        #[arg(short, long, help = "Force stop the VM (power-off)")]
        force: bool,
    },

    /// Gracefully restart VM
    Restart { id: Uuid },

    /// Delete VM and all its files
    Delete { id: Uuid },

    /// Check if VM spec matches the current state
    Check { id: Uuid },

    /// Get VM status: running, stopped
    Status { id: Uuid },
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

fn default_vm_store() -> PathBuf {
    // if launched through sudo SUDO_UID will be our user
    if let Ok(uid_str) = env::var("SUDO_UID") {
        if let Ok(uid) = uid_str.parse::<u32>() {
            if let Some(u) = get_user_by_uid(uid) {
                return u.home_dir().join(DEFAULT_VM_STORE);
            }
        }
    }

    if let Some(u) = get_user_by_uid(get_current_uid()) {
        return u.home_dir().join(DEFAULT_VM_STORE);
    }

    // fallback to the current directory
    PathBuf::from(DEFAULT_VM_STORE)
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
            let details = IsoMaker.create_application_iso(spec, &output).await.context("creating ISO")?;
            Ok(ActionOutput { status: "created".into(), details })
        }
    }
}

async fn run_vm_command(args: VmCommandArgs) -> Result<ActionOutput<VmDetails>, QemuClientError> {
    let client = QemuClient::new(args.qemu_system_bin, args.qemu_img_bin, args.vm_store);
    match args.command {
        VmCommand::Create {
            id,
            cpu,
            ram_mib,
            disk_gib,
            cdrom_iso_path,
            gpu,
            port_forward,
            bios_path,
            kernel_path,
            initrd_path,
            kernel_args,
            display_gtk,
            enable_cvm,
        } => {
            let pf = parse_port_forward(&port_forward).expect("TODO");

            let spec = VmSpec {
                cpu,
                ram_mib,
                hard_disks: vec![HardDiskSpec::Create { gib: disk_gib, format: HardDiskFormat::Qcow2 }],
                cdrom_iso_path,
                gpu_enabled: gpu,
                port_forwarding: pf,
                bios_path,
                kernel_path,
                initrd_path,
                kernel_args,
                display_gtk,
                enable_cvm,
            };

            client.create_vm(id, spec).await.map(|details| ActionOutput { status: "created".into(), details })
        }
        VmCommand::Start { id } => {
            client.start_vm(id).await.map(|details| ActionOutput { status: "started".into(), details })
        }
        VmCommand::Stop { id, force } => {
            client.stop_vm(id, force).await.map(|details| ActionOutput { status: "stopped".into(), details })
        }
        VmCommand::Restart { id } => {
            client.restart_vm(id).await.map(|details| ActionOutput { status: "restarted".into(), details })
        }
        VmCommand::Delete { id } => {
            client.delete_vm(id).await.map(|details| ActionOutput { status: "deleted".into(), details })
        }
        VmCommand::Check { id } => {
            client.check_vm_spec(id).await.map(|details| ActionOutput { status: "matching".into(), details })
        }
        VmCommand::Status { id } => client.vm_status(id).await.map(|(details, running)| ActionOutput {
            status: if running { "running".into() } else { "stopped".into() },
            details,
        }),
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
    fs::create_dir_all(&config.vm_store).context("Failed to create VM store directory")?;

    let api_client = Box::new(RestNilccApiClient::new(endpoint, key)?);
    let args = AgentServiceArgs { agent_id: config.agent_id, api_client, sync_interval };
    let mut agent_service = AgentService::new(args);
    debug!("AgentService is running.");

    tokio::signal::ctrl_c().await?;
    debug!("Ctrl+C received. Initiating graceful shutdown of AgentService.");
    agent_service.request_shutdown();

    Ok(())
}

async fn run(cli: Cli) -> anyhow::Result<Box<dyn SerializeAsAny>> {
    let Cli { command } = cli;
    match command {
        Command::Vm { args } => Ok(Box::new(run_vm_command(args).await?)),
        Command::Iso(command) => Ok(Box::new(run_iso_command(command).await?)),
        Command::Daemon { config } => {
            let agent_config = load_config(config).context("Loading agent configuration")?;
            run_daemon(agent_config).await?;
            Ok(Box::new(()))
        }
        Command::Info => {
            let instance_details = nilcc_agent::agent_service::gather_metal_instance_details().await?;
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

/// Parse `HOST:VM` rules supplied via `--portfwd`. Supports multiple flags.
fn parse_port_forward(rules: &[String]) -> Result<Vec<(u16, u16)>> {
    let mut pairs = Vec::new();
    for rule in rules {
        let (host, vm) =
            rule.split_once(':').ok_or_else(|| anyhow::anyhow!("Invalid port mapping '{rule}', expected HOST:VM"))?;
        let host: u16 = host.parse().map_err(|_| anyhow::anyhow!("Invalid host port in '{rule}'"))?;
        let vm: u16 = vm.parse().map_err(|_| anyhow::anyhow!("Invalid guest port in '{rule}'"))?;
        pairs.push((host, vm));
    }
    Ok(pairs)
}
