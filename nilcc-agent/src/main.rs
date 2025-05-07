mod output;
mod qemu_client;

use crate::qemu_client::VmDetails;
use anyhow::Result;
use clap::{Parser, Subcommand};
use output::{serialize_error, serialize_output};
use qemu_client::{QemuClient, VmSpec};
use serde::Serialize;
use std::path::PathBuf;

const DEFAULT_QEMU_SYSTEM: &str = "/usr/bin/qemu-system-x86_64";
const DEFAULT_QEMU_IMG: &str = "/usr/bin/qemu-img";

#[derive(Parser)]
struct Cli {
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

    /// Command
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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

fn default_vm_store() -> PathBuf {
    dirs::home_dir().expect("Unable to resolve $HOME").join(".nilcc/vms")
}

/// JSON wrapper for success responses
#[derive(Serialize)]
struct VmActionOutput {
    status: String,
    details: VmDetails,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let cli = Cli::parse();
    let client = QemuClient::new(&cli.qemu_system_bin, &cli.qemu_img_bin);

    let result = match cli.command {
        Command::Create { name, cpu, ram_mib, disk_gib, cdrom_iso_path, gpu, port_forward, bios_path, display_gtk } => {
            let pf = parse_port_forward(&port_forward)?;

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

            client
                .create_vm(&cli.vm_store, &name, spec)
                .await
                .map(|details| VmActionOutput { status: "created".into(), details })
        }

        Command::Start { name } => client
            .start_vm(&cli.vm_store, &name)
            .await
            .map(|details| VmActionOutput { status: "started".into(), details }),

        Command::Stop { name } => client
            .stop_vm(&cli.vm_store, &name)
            .await
            .map(|details| VmActionOutput { status: "stopped".into(), details }),

        Command::Delete { name } => client
            .delete_vm(&cli.vm_store, &name)
            .await
            .map(|details| VmActionOutput { status: "deleted".into(), details }),

        Command::Check { name } => client
            .check_vm_spec(&cli.vm_store, &name)
            .await
            .map(|details| VmActionOutput { status: "matching".into(), details }),

        Command::Status { name } => client.vm_status(&cli.vm_store, &name).await.map(|(details, running)| {
            VmActionOutput { status: if running { "running".into() } else { "stopped".into() }, details }
        }),
    };

    match result {
        Ok(val) => println!("{}", serialize_output(&val).unwrap()),
        Err(e) => {
            eprintln!("{}", serialize_error(&e.into()));
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
