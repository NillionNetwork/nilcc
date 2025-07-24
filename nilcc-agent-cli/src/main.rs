use crate::api::ApiClient;
use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use nilcc_agent_models::workloads::{
    create::{CreateWorkloadRequest, CreateWorkloadResponse},
    delete::DeleteWorkloadRequest,
};
use std::{fs, path::PathBuf, process::exit, str::FromStr};
use uuid::Uuid;

mod api;

#[derive(Parser)]
struct Cli {
    #[clap(long, env = "NILCC_AGENT_URL")]
    url: String,

    #[clap(long, env = "NILCC_AGENT_API_KEY")]
    api_key: String,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Launch a workload.
    Launch(LaunchArgs),

    /// Delete a workload.
    Delete(DeleteArgs),

    /// Start a workload
    Start(StartArgs),

    /// Stop a workload.
    Stop(StopArgs),

    /// Restart a workload.
    Restart(RestartArgs),
}

#[derive(Args)]
struct LaunchArgs {
    /// The id to use for the workload.
    #[clap(long)]
    id: Option<Uuid>,

    /// The environment variables to be set, in the format `<name>=<value>`.
    #[clap(short, long = "env-var")]
    env_vars: Vec<KeyValue>,

    /// The files to be included in the ISO image, in the format `<name>=<value>`.
    #[clap(short, long = "file")]
    files: Vec<KeyValue>,

    /// The container entrypoint, in the format `<container-name>:<container-port>`
    #[clap(long)]
    entrypoint: Entrypoint,

    /// The number of CPUs to use in the VM.
    #[clap(long, default_value_t = 1)]
    cpus: u32,

    /// The number of GPUs to use in the VM.
    #[clap(long, default_value_t = 0)]
    gpus: u16,

    /// The amount of RAM, in MBs.
    #[clap(long, default_value_t = 2048)]
    memory_mb: u32,

    /// The amount of disk space, in GBs, to use for the VM's state disk.
    #[clap(long = "disk-space", default_value_t = 10)]
    disk_space_gb: u32,

    /// The domain for the VM.
    #[clap(long)]
    domain: String,

    /// The path to the docker compose file to be used.
    #[clap(long = "docker-compose")]
    docker_compose_path: PathBuf,
}

#[derive(Args)]
struct DeleteArgs {
    /// The identifier of the workload to be deleted.
    id: Uuid,
}

#[derive(Args)]
struct StopArgs {
    /// The identifier of the workload to be stopped.
    id: Uuid,
}

#[derive(Args)]
struct StartArgs {
    /// The identifier of the workload to be started.
    id: Uuid,
}

#[derive(Args)]
struct RestartArgs {
    /// The identifier of the workload to be restarted.
    id: Uuid,
}

#[derive(Clone)]
struct KeyValue {
    key: String,
    value: String,
}

impl FromStr for KeyValue {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (key, value) = s.split_once('=').ok_or("missing '='")?;
        let key = key.to_string();
        let value = value.to_string();
        Ok(Self { key, value })
    }
}

#[derive(Clone)]
struct Entrypoint {
    container: String,
    port: u16,
}

impl FromStr for Entrypoint {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (container, port) = s.split_once(':').ok_or("missing ':'")?;
        let container = container.to_string();
        let port = port.parse().map_err(|_| "invalid port")?;
        Ok(Self { container, port })
    }
}

fn launch(client: ApiClient, args: LaunchArgs) -> anyhow::Result<()> {
    let LaunchArgs {
        id,
        env_vars,
        files,
        entrypoint,
        cpus,
        gpus,
        memory_mb,
        disk_space_gb,
        domain,
        docker_compose_path,
    } = args;
    let docker_compose = fs::read_to_string(docker_compose_path).context("Failed to read docker compose")?;
    let env_vars = env_vars.into_iter().map(|kv| (kv.key, kv.value)).collect();
    let files = files
        .into_iter()
        .map(|f| fs::read(&f.value).map(|contents| (f.key, contents)).context("Failed to read file"))
        .collect::<Result<_, _>>()?;
    let request = CreateWorkloadRequest {
        id: id.unwrap_or_else(Uuid::new_v4),
        docker_compose,
        env_vars,
        files,
        public_container_name: entrypoint.container,
        public_container_port: entrypoint.port,
        memory_mb,
        cpus,
        gpus,
        disk_space_gb,
        domain,
    };
    let response: CreateWorkloadResponse = client.post("/api/v1/workloads/create", &request)?;
    let CreateWorkloadResponse { id } = response;
    println!("Workload id {id} launched");
    Ok(())
}

fn delete(client: ApiClient, args: DeleteArgs) -> anyhow::Result<()> {
    let DeleteArgs { id } = args;
    let request = DeleteWorkloadRequest { id };
    let _: () = client.post("/api/v1/workloads/delete", &request)?;
    println!("Workload {id} deleted");
    Ok(())
}

fn start(client: ApiClient, args: StartArgs) -> anyhow::Result<()> {
    let StartArgs { id } = args;
    let request = DeleteWorkloadRequest { id };
    let _: () = client.post("/api/v1/workloads/start", &request)?;
    println!("Workload {id} started");
    Ok(())
}

fn stop(client: ApiClient, args: StopArgs) -> anyhow::Result<()> {
    let StopArgs { id } = args;
    let request = DeleteWorkloadRequest { id };
    let _: () = client.post("/api/v1/workloads/stop", &request)?;
    println!("Workload {id} stopped");
    Ok(())
}

fn restart(client: ApiClient, args: RestartArgs) -> anyhow::Result<()> {
    let RestartArgs { id } = args;
    let request = DeleteWorkloadRequest { id };
    let _: () = client.post("/api/v1/workloads/restart", &request)?;
    println!("Workload {id} restarted");
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let Cli { url, api_key, command } = cli;

    let client = ApiClient::new(url, &api_key);
    let result = match command {
        Command::Launch(args) => launch(client, args),
        Command::Delete(args) => delete(client, args),
        Command::Start(args) => start(client, args),
        Command::Stop(args) => stop(client, args),
        Command::Restart(args) => restart(client, args),
    };
    if let Err(e) = result {
        eprintln!("Failed to run command: {e:#}");
        exit(1);
    }
}
