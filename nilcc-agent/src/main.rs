mod iso;
mod output;
mod qemu_client;

use crate::qemu_client::VmDetails;
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use iso::{ApplicationMetadata, ContainerMetadata, IsoMaker, IsoSpec};
use output::{serialize_error, serialize_output, SerializeAsAny};
use qemu_client::{QemuClient, QemuClientError, VmSpec};
use serde::Serialize;
use std::{ops::Deref, path::PathBuf};

const DEFAULT_QEMU_SYSTEM: &str = "/usr/bin/qemu-system-x86_64";
const DEFAULT_QEMU_IMG: &str = "/usr/bin/qemu-img";

#[derive(Parser)]
struct Cli {
    #[clap(flatten)]
    configs: Configs,

    /// The command to be ran.
    #[command(subcommand)]
    command: Command,
}

#[derive(Args)]
struct Configs {
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
}

#[derive(Subcommand)]
enum Command {
    /// VM commands.
    #[clap(subcommand)]
    Vm(VmCommand),

    /// ISO commands.
    #[clap(subcommand)]
    Iso(IsoCommand),
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
    },

    /// Start already created VM
    Start { name: String },

    /// Stop running VM
    Stop { name: String },

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

fn default_vm_store() -> PathBuf {
    dirs::home_dir().expect("Unable to resolve $HOME").join(".nilcc/vms")
}

/// JSON wrapper for VM success responses
#[derive(Serialize)]
struct VmActionOutput {
    status: String,
    details: VmDetails,
}

/// JSON wrapper for ISO success responses
#[derive(Serialize)]
struct IsoActionOutput {
    status: String,
}

async fn run_iso_command(command: IsoCommand) -> Result<IsoActionOutput> {
    match command {
        IsoCommand::Create { container, port, hostname, output, docker_compose_path } => {
            let compose = std::fs::read_to_string(docker_compose_path).context("reading docker compose")?;
            let spec = IsoSpec {
                docker_compose_yaml: compose,
                metadata: ApplicationMetadata { hostname, api: ContainerMetadata { container, port } },
            };
            IsoMaker.create_application_iso(spec, &output).await.context("creating ISO")?;
            Ok(IsoActionOutput { status: "ISO created".into() })
        }
    }
}

async fn run_vm_command(configs: Configs, command: VmCommand) -> Result<VmActionOutput, QemuClientError> {
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
            };

            client.create_vm(&name, spec).await.map(|details| VmActionOutput { status: "created".into(), details })
        }
        VmCommand::Start { name } => {
            client.start_vm(&name).await.map(|details| VmActionOutput { status: "started".into(), details })
        }
        VmCommand::Stop { name } => {
            client.stop_vm(&name).await.map(|details| VmActionOutput { status: "stopped".into(), details })
        }
        VmCommand::Delete { name } => {
            client.delete_vm(&name).await.map(|details| VmActionOutput { status: "deleted".into(), details })
        }
        VmCommand::Check { name } => {
            client.check_vm_spec(&name).await.map(|details| VmActionOutput { status: "matching".into(), details })
        }
        VmCommand::Status { name } => client.vm_status(&name).await.map(|(details, running)| VmActionOutput {
            status: if running { "running".into() } else { "stopped".into() },
            details,
        }),
    }
}

async fn run(cli: Cli) -> anyhow::Result<Box<dyn SerializeAsAny>> {
    let Cli { configs, command } = cli;
    match command {
        Command::Vm(command) => Ok(Box::new(run_vm_command(configs, command).await?)),
        Command::Iso(command) => Ok(Box::new(run_iso_command(command).await?)),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

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
