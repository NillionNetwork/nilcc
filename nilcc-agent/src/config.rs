use clap::Args;
use serde::Deserialize;
use std::{env, path::PathBuf, time::Duration};
use users::{get_current_uid, get_user_by_uid, os::unix::UserExt};
use uuid::Uuid;

const DEFAULT_QEMU_SYSTEM: &str = "qemu-system-x86_64";
const DEFAULT_QEMU_IMG: &str = "qemu-img";
const DEFAULT_VM_STORE: &str = ".nilcc/vms";

fn default_qemu_system_bin() -> PathBuf {
    PathBuf::from(DEFAULT_QEMU_SYSTEM)
}

fn default_qemu_img_bin() -> PathBuf {
    PathBuf::from(DEFAULT_QEMU_IMG)
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

#[derive(Args, Deserialize, Debug)]
pub struct AgentConfig {
    /// Directory where VM folders live (default: $HOME/.nilcc/vms)
    #[clap(env, long, short, default_value_os_t = default_vm_store())]
    pub vm_store: PathBuf,

    /// Optional qemu-system binary
    #[clap(
        long = "qemu-system-bin",
        env = "VM_QEMU_SYSTEM_BIN",
        default_value = DEFAULT_QEMU_SYSTEM
    )]
    #[serde(default = "default_qemu_system_bin")]
    pub qemu_system_bin: PathBuf,

    /// Optional; qemu-img binary
    #[clap(
        long = "qemu-img-bin",
        env = "VM_QEMU_IMG_BIN",
        default_value = DEFAULT_QEMU_IMG
    )]
    #[serde(default = "default_qemu_img_bin")]
    pub qemu_img_bin: PathBuf,

    /// Unique agent ID, used to identify this agent in nilCC server
    #[clap(long = "agent-id", short = 'i')]
    pub agent_id: Option<Uuid>,

    /// nilCC API endpoint to connect to when running as daemon
    #[clap(long = "nilcc-api-endpoint", short = 'e', env = "NILCC_API_ENDPOINT")]
    pub nilcc_api_endpoint: Option<String>,

    /// nilCC API key
    #[clap(long = "nilcc-api-key", short = 'k', env = "NILCC_API_KEY")]
    pub nilcc_api_key: Option<String>,

    /// Interval for periodic synchronization task, defaults to 10 seconds
    #[clap(long = "sync-interval", short = 's', default_value = "10sec", value_parser = humantime::parse_duration)]
    #[serde(with = "humantime_serde::option")]
    pub sync_interval: Option<Duration>,
}
