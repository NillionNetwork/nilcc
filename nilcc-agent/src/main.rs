mod iso;
mod output;
mod qemu_client;

use crate::{output::RunOutput, qemu_client::VmDetails};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use iso::{ApplicationMetadata, ContainerMetadata, IsoMaker, IsoMetadata, IsoSpec};
use nilcc_agent::agent_service::AgentService;
use output::{serialize_error, serialize_output};
use qemu_client::{QemuClient, QemuClientError, VmSpec};
use serde::{Deserialize, Serialize};
use std::{env, fs, ops::Deref, path::PathBuf};
use tracing::{debug, info};
use users::{get_current_uid, get_user_by_uid, os::unix::UserExt};

const DEFAULT_QEMU_SYSTEM: &str = "qemu-system-x86_64";
const DEFAULT_QEMU_IMG: &str = "qemu-img";
const DEFAULT_VM_STORE: &str = ".nilcc/vms";

fn default_qemu_system_bin() -> PathBuf {
    PathBuf::from(DEFAULT_QEMU_SYSTEM)
}

fn default_qemu_img_bin() -> PathBuf {
    PathBuf::from(DEFAULT_QEMU_IMG)
}

#[derive(Parser)]
struct Cli {
    #[clap(flatten)]
    configs: AgentConfig,

    /// The command to be ran.
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Deserialize, Debug)]
struct AgentConfig {
    /// Directory where VM folders live (default: $HOME/.nilcc/vms)
    #[clap(env, long, short, default_value_os_t = default_vm_store())]
    vm_store: PathBuf,

    /// Optional qemu-system binary
    #[clap(
        long = "qemu-system-bin",
        env = "VM_QEMU_SYSTEM_BIN",
        default_value = DEFAULT_QEMU_SYSTEM
    )]
    #[serde(default = "default_qemu_system_bin")]
    qemu_system_bin: PathBuf,

    /// Optional; qemu-img binary
    #[clap(
        long = "qemu-img-bin",
        env = "VM_QEMU_IMG_BIN",
        default_value = DEFAULT_QEMU_IMG
    )]
    #[serde(default = "default_qemu_img_bin")]
    qemu_img_bin: PathBuf,

    /// nilCC API endpoint to connect to when running as daemon
    #[clap(long = "nilcc-api-endpoint", short = 'e', env = "NILCC_API_ENDPOINT")]
    nilcc_api_endpoint: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// VM commands.
    #[clap(subcommand)]
    Vm(VmCommand),

    /// ISO commands.
    #[clap(subcommand)]
    Iso(IsoCommand),

    /// Run the agent in daemon mode and connect to nilCC API.
    #[clap(name = "daemon")]
    Daemon {
        /// Path to the agent configuration file
        #[clap(long, short, default_value = "nilcc-agent-config.yaml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum VmCommand {
    /// Create and start a fresh VM
    Create {
        /// Unique name of the VM
        name: String,

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

        /// Optional Path to BIOS file
        #[clap(long = "bios-path")]
        bios_path: Option<PathBuf>,

        /// Show GTK ;display for VM, defaults to headless VM
        #[clap(long = "display-gtk")]
        display_gtk: bool,

        /// Enable CVM
        #[clap(long = "enable-cvm")]
        enable_cvm: bool,
    },

    /// Start already created VM
    Start { name: String },

    /// Gracefully stop running VM or force stop if `force` is true
    Stop {
        name: String,
        #[arg(short, long, help = "Force stop the VM (power-off)")]
        force: bool,
    },

    /// Gracefully restart VM
    Restart { name: String },

    /// Delete VM and all its files
    Delete { name: String },

    /// Check if VM spec matches the current state
    Check { name: String },

    /// Get VM status: running, stopped
    Status { name: String },
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

        /// The path to the docker compose to be ran.
        docker_compose_path: PathBuf,
    },
}

/// Resolve ~/.nilcc/vms
pub fn default_vm_store() -> PathBuf {
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
        IsoCommand::Create { container, port, hostname, output, docker_compose_path } => {
            let compose = std::fs::read_to_string(docker_compose_path).context("reading docker compose")?;
            let spec = IsoSpec {
                docker_compose_yaml: compose,
                metadata: ApplicationMetadata { hostname, api: ContainerMetadata { container, port } },
            };
            let details = IsoMaker.create_application_iso(spec, &output).await.context("creating ISO")?;
            Ok(ActionOutput { status: "created".into(), details })
        }
    }
}

async fn run_vm_command(configs: AgentConfig, command: VmCommand) -> Result<ActionOutput<VmDetails>, QemuClientError> {
    let client = QemuClient::new(configs.qemu_system_bin, configs.qemu_img_bin, configs.vm_store);
    match command {
        VmCommand::Create {
            name,
            cpu,
            ram_mib,
            disk_gib,
            cdrom_iso_path,
            gpu,
            port_forward,
            bios_path,
            display_gtk,
            enable_cvm,
        } => {
            let pf = parse_port_forward(&port_forward).expect("TODO");

            let spec = VmSpec {
                cpu,
                ram_mib,
                disk_gib,
                cdrom_iso_path,
                gpu_enabled: gpu,
                port_forwarding: pf,
                bios_path,
                display_gtk,
                enable_cvm,
            };

            client.create_vm(&name, spec).await.map(|details| ActionOutput { status: "created".into(), details })
        }
        VmCommand::Start { name } => {
            client.start_vm(&name).await.map(|details| ActionOutput { status: "started".into(), details })
        }
        VmCommand::Stop { name, force } => {
            client.stop_vm(&name, force).await.map(|details| ActionOutput { status: "stopped".into(), details })
        }
        VmCommand::Restart { name } => {
            client.restart_vm(&name).await.map(|details| ActionOutput { status: "restarted".into(), details })
        }
        VmCommand::Delete { name } => {
            client.delete_vm(&name).await.map(|details| ActionOutput { status: "deleted".into(), details })
        }
        VmCommand::Check { name } => {
            client.check_vm_spec(&name).await.map(|details| ActionOutput { status: "matching".into(), details })
        }
        VmCommand::Status { name } => client.vm_status(&name).await.map(|(details, running)| ActionOutput {
            status: if running { "running".into() } else { "stopped".into() },
            details,
        }),
    }
}

async fn run_daemon(config: AgentConfig) -> Result<()> {
    let api_endpoint = config
        .nilcc_api_endpoint
        .ok_or_else(|| anyhow::anyhow!("nilCC API endpoint is required when running in daemon mode"))?;

    debug!("Connecting to nilCC API endpoint: {}", api_endpoint);

    tokio::spawn(async move { AgentService::connect(api_endpoint).await.context("connecting to nilCC API") });

    Ok(())
}

fn load_config(config_path: PathBuf) -> Result<AgentConfig> {
    debug!("Loading configuration from: {}", config_path.display());

    let config_file = fs::File::open(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to open config file {}: {}", config_path.display(), e))?;

    let config: AgentConfig = serde_yaml::from_reader(config_file)
        .map_err(|e| anyhow::anyhow!("Failed to parse YAML from config file {}: {}", config_path.display(), e))?;

    Ok(config)
}

async fn run(cli: Cli) -> anyhow::Result<RunOutput> {
    let Cli { configs, command } = cli;

    match command {
        Command::Vm(command) => {
            let output = run_vm_command(configs, command).await?;
            Ok(RunOutput::Data(Box::new(output)))
        }
        Command::Iso(command) => {
            let output = run_iso_command(command).await?;
            Ok(RunOutput::Data(Box::new(output)))
        }
        Command::Daemon { config: config_path } => {
            let app_config = load_config(config_path).context("loading agent configuration")?;
            run_daemon(app_config).await?;
            Ok(RunOutput::DaemonSuccessfullyStarted)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    let cli = Cli::parse();
    match run(cli).await {
        Ok(output) => match output {
            RunOutput::DaemonSuccessfullyStarted => {
                info!("Daemon successfully started");
                if let Err(e) = tokio::signal::ctrl_c().await {
                    eprintln!("Failed to listen for Ctrl+C signal: {}", e);
                }
            }
            RunOutput::Data(val) => {
                println!("{}", serialize_output(val.deref())?);
            }
        },

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
