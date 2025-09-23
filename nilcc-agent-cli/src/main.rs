use crate::api::ApiClient;
use ansi_term::Color;
use anyhow::anyhow;
use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use cvm_agent_models::health::HealthResponse;
use cvm_agent_models::health::LastEvent;
use cvm_agent_models::logs::SystemLogsRequest;
use cvm_agent_models::logs::SystemLogsResponse;
use cvm_agent_models::logs::SystemLogsSource;
use cvm_agent_models::stats::CpuStats;
use cvm_agent_models::stats::DiskStats;
use cvm_agent_models::stats::SystemStatsResponse;
use cvm_agent_models::{
    container::Container,
    logs::{ContainerLogsRequest, ContainerLogsResponse, OutputStream},
};
use nilcc_agent_models::system::ArtifactsCleanupResponse;
use nilcc_agent_models::system::InstallArtifactVersionRequest;
use nilcc_agent_models::system::LastUpgrade;
use nilcc_agent_models::system::UpgradeState;
use nilcc_agent_models::system::VersionResponse;
use nilcc_agent_models::workloads::{
    create::{CreateWorkloadRequest, CreateWorkloadResponse},
    delete::DeleteWorkloadRequest,
    list::WorkloadSummary,
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::exit,
    str::FromStr,
};
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

    /// List workloads.
    List,

    /// Delete a workload.
    Delete(DeleteArgs),

    /// Check the health for a workload.
    Health(HealthArgs),

    /// Start a workload
    Start(StartArgs),

    /// Stop a workload.
    Stop(StopArgs),

    /// Restart a workload.
    Restart(RestartArgs),

    /// Container commands.
    #[clap(subcommand)]
    Containers(ContainersCommand),

    /// System commands.
    #[clap(subcommand)]
    System(SystemCommand),

    /// Admin commands
    #[clap(subcommand)]
    Admin(AdminCommand),
}

#[derive(Subcommand)]
enum ContainersCommand {
    /// List the running containers in
    List(ListContainersArgs),

    /// Get logs for a container.
    Logs(ContainerLogsArgs),
}

#[derive(Subcommand)]
enum SystemCommand {
    /// Get system level logs.
    Logs(SystemLogsArgs),

    /// Get system stats.
    Stats(SystemStatsArgs),
}

#[derive(Subcommand)]
enum AdminCommand {
    /// Manage artifact versions.
    #[clap(subcommand)]
    Artifacts(AdminArtifactsCommand),

    /// Manage nilcc-agent versions.
    #[clap(subcommand)]
    Agent(AdminAgentCommand),
}

#[derive(Subcommand)]
enum AdminArtifactsCommand {
    /// Install a new artifacts version.
    Install(InstallArtifactsArgs),

    /// Get the current artifacts version.
    Version,

    /// Cleanup unused artifact versions.
    Cleanup,
}

#[derive(Subcommand)]
enum AdminAgentCommand {
    /// Upgrade the nilcc-agent binary.
    Upgrade(UpgradeAgentArgs),

    /// Get the current nilcc-agent binary version.
    Version,
}

#[derive(Args)]
struct LaunchArgs {
    /// The id to use for the workload.
    #[clap(long)]
    id: Option<Uuid>,

    /// The artifacts version to use.
    #[clap(short, long)]
    artifacts: String,

    /// Add an environment variable to the workload, in the format `<name>=<value>`.
    #[clap(short, long = "env-var")]
    env_vars: Vec<KeyValue>,

    /// The path to a .env file to add environment variables from.
    #[clap(long = "dotenv-file")]
    dotenv: Option<PathBuf>,

    /// Add a file to the workload, in the format `<file-name>=<value>`.
    #[clap(short, long = "file")]
    files: Vec<KeyValue>,

    /// Add docker credentials, in the format `<server>:<username>:<password>`
    #[clap(long)]
    docker_credentials: Vec<DockerCredentials>,

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

#[derive(Args)]
struct ListContainersArgs {
    /// The identifier of the workload to list containers for.
    id: Uuid,
}

#[derive(Args)]
struct ContainerLogsArgs {
    /// The identifier of the workload to get logs from.
    id: Uuid,

    /// The container to pull logs from
    #[clap(short, long)]
    container: String,

    /// Whether to get stderr logs. By default stdout logs are fetched.
    #[clap(long)]
    stderr: bool,

    /// Whether to fetch logs from the head of the stream. By default logs are fetched from the
    /// tail.
    #[clap(long)]
    head: bool,

    /// The maximum number of lines to get.
    #[clap(long, default_value_t = 1000)]
    max_lines: usize,
}

#[derive(Args)]
struct SystemLogsArgs {
    /// The identifier of the workload to get logs from.
    id: Uuid,

    /// Whether to fetch logs from the head of the stream. By default logs are fetched from the
    /// tail.
    #[clap(long)]
    head: bool,

    /// The maximum number of lines to get.
    #[clap(long, default_value_t = 1000)]
    max_lines: usize,
}

#[derive(Args)]
struct SystemStatsArgs {
    /// The identifier of the workload to get stats from.
    id: Uuid,
}

#[derive(Args)]
struct HealthArgs {
    /// The identifier of the workload to get health stats from.
    id: Uuid,
}

#[derive(Args)]
struct InstallArtifactsArgs {
    /// The artifact version to update to.
    version: String,
}

#[derive(Args)]
struct UpgradeAgentArgs {
    /// The agent version to update to.
    version: String,
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

#[derive(Clone)]
struct DockerCredentials {
    server: String,
    username: String,
    password: String,
}

impl FromStr for DockerCredentials {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err("not enough ':' in docker credentials");
        }
        let server = parts[0].into();
        let username = parts[1].into();
        let password = parts[2].into();
        Ok(Self { server, username, password })
    }
}

fn load_dotenv(path: &Path) -> anyhow::Result<HashMap<String, String>> {
    let file = File::open(path).context("Failed to open .env file")?;
    let reader = BufReader::new(file);
    let mut output = HashMap::new();
    for line in reader.lines() {
        let line = line.context("Failed to read .env file")?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once("=").ok_or_else(|| anyhow!("no '=' in .env line"))?;
        output.insert(name.to_string(), value.to_string());
    }
    Ok(output)
}

fn launch(client: ApiClient, args: LaunchArgs) -> anyhow::Result<()> {
    let LaunchArgs {
        id,
        artifacts,
        env_vars,
        dotenv,
        files,
        docker_credentials,
        entrypoint,
        cpus,
        gpus,
        memory_mb,
        disk_space_gb,
        domain,
        docker_compose_path,
    } = args;
    let docker_compose = fs::read_to_string(docker_compose_path).context("Failed to read docker compose")?;
    let mut env_vars: HashMap<_, _> = env_vars.into_iter().map(|kv| (kv.key, kv.value)).collect();
    if let Some(dotenv) = dotenv {
        env_vars.extend(load_dotenv(&dotenv)?);
    }
    let files = files
        .into_iter()
        .map(|f| fs::read(&f.value).map(|contents| (f.key, contents)).context("Failed to read file"))
        .collect::<Result<_, _>>()?;
    let request = CreateWorkloadRequest {
        id: id.unwrap_or_else(Uuid::new_v4),
        artifacts_version: artifacts,
        docker_compose,
        env_vars,
        files,
        docker_credentials: docker_credentials
            .into_iter()
            .map(|c| nilcc_agent_models::workloads::create::DockerCredentials {
                server: c.server,
                username: c.username,
                password: c.password,
            })
            .collect(),
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
    println!("Workload {id} launched");
    Ok(())
}

fn list(client: ApiClient) -> anyhow::Result<()> {
    let workloads: Vec<WorkloadSummary> = client.get("/api/v1/workloads/list")?;
    let containers = serde_json::to_string_pretty(&workloads).expect("failed to serialize");
    println!("{containers}");
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

fn health(client: ApiClient, args: HealthArgs) -> anyhow::Result<()> {
    let HealthArgs { id } = args;
    let response: HealthResponse = client.get(&format!("/api/v1/workloads/{id}/health"))?;
    let HealthResponse { https, bootstrapped, last_event, .. } = response;
    let color = bool_to_color(bootstrapped);
    println!("bootstrapped: {}", color.paint(bootstrapped.to_string()));

    let color = bool_to_color(https);
    println!("https up:     {}", color.paint(https.to_string()));

    if let Some(last_event) = last_event {
        let LastEvent { message, timestamp, kind, .. } = last_event;
        let text = format!("cvm reported {kind:?} event at {timestamp}: {message}");
        println!("{}", Color::Red.paint(text));
    }
    Ok(())
}

fn list_containers(client: ApiClient, args: ListContainersArgs) -> anyhow::Result<()> {
    let ListContainersArgs { id } = args;
    let containers: Vec<Container> = client.get(&format!("/api/v1/workloads/{id}/containers/list"))?;
    let containers = serde_json::to_string_pretty(&containers).expect("failed to serialize");
    println!("{containers}");
    Ok(())
}

fn container_logs(client: ApiClient, args: ContainerLogsArgs) -> anyhow::Result<()> {
    let ContainerLogsArgs { id, container, head, stderr, max_lines } = args;
    let stream = if stderr { OutputStream::Stderr } else { OutputStream::Stdout };
    let request = ContainerLogsRequest { container, tail: !head, stream, max_lines };
    let response: ContainerLogsResponse =
        client.get_query(&format!("/api/v1/workloads/{id}/containers/logs"), &request)?;
    for line in response.lines {
        println!("{line}");
    }
    Ok(())
}

fn system_logs(client: ApiClient, args: SystemLogsArgs) -> anyhow::Result<()> {
    let SystemLogsArgs { id, head, max_lines } = args;
    let request = SystemLogsRequest { tail: !head, max_lines, source: SystemLogsSource::CvmAgent };
    let response: SystemLogsResponse = client.get_query(&format!("/api/v1/workloads/{id}/system/logs"), &request)?;
    for line in response.lines {
        println!("{line}");
    }
    Ok(())
}

fn system_stats(client: ApiClient, args: SystemStatsArgs) -> anyhow::Result<()> {
    let SystemStatsArgs { id } = args;
    let response: SystemStatsResponse = client.get(&format!("/api/v1/workloads/{id}/system/stats"))?;
    let SystemStatsResponse { memory, cpus, disks } = response;
    let memory_total = bytes_to_mb(memory.total);
    let memory_used = bytes_to_mb(memory.used);
    let color = percent_to_color((memory_used as f64) / (memory_total as f64));
    let details = format!("{memory_used}MB/{memory_total}MB");

    println!("Mem usage: {}", color.paint(details));
    println!("CPU usage:");
    for cpu in cpus {
        let CpuStats { name, usage, frequency } = cpu;
        let color = percent_to_color((usage / 100.0).into());
        let details = format!("{usage:.1}%");
        println!("* {name} ({frequency} MHz): {}", color.paint(details));
    }
    println!("Disks:");
    for disk in disks {
        let DiskStats { name, mount_point, filesystem, size, used } = disk;
        let mount_point = mount_point.display();
        let color = percent_to_color(used as f64 / size as f64);
        let details = format!("{:.2}GB/{:.2}GB", bytes_to_gb(used), bytes_to_gb(size));
        println!("* {name} mounted at {mount_point} ({filesystem}): {}", color.paint(details));
    }
    Ok(())
}

fn install_artifacts(client: ApiClient, args: InstallArtifactsArgs) -> anyhow::Result<()> {
    let InstallArtifactsArgs { version } = args;
    let request = InstallArtifactVersionRequest { version: version.clone() };
    let _: () = client.post("/api/v1/system/artifacts/install", &request)?;
    println!("Installation of version {version} scheduled");
    Ok(())
}

fn artifacts_version(client: ApiClient) -> anyhow::Result<()> {
    let response: VersionResponse = client.get("/api/v1/system/artifacts/version")?;
    display_version(response)
}

fn cleanup_artifacts(client: ApiClient) -> anyhow::Result<()> {
    let ArtifactsCleanupResponse { versions_deleted } = client.post("/api/v1/system/artifacts/cleanup", &())?;
    if versions_deleted.is_empty() {
        println!("No versions deleted");
    } else {
        println!("{} versions deleted: ", versions_deleted.len());
        for version in versions_deleted {
            println!("- {version}");
        }
    }
    Ok(())
}

fn upgrade_agent(client: ApiClient, args: UpgradeAgentArgs) -> anyhow::Result<()> {
    let UpgradeAgentArgs { version } = args;
    let request = InstallArtifactVersionRequest { version: version.clone() };
    let _: () = client.post("/api/v1/system/agent/upgrade", &request)?;
    println!("Upgrade to version {version} scheduled");
    Ok(())
}

fn agent_version(client: ApiClient) -> anyhow::Result<()> {
    let response: VersionResponse = client.get("/api/v1/system/agent/version")?;
    display_version(response)
}

fn display_version(response: VersionResponse) -> anyhow::Result<()> {
    let VersionResponse { version, last_upgrade } = response;
    println!("Version: {version}");
    match last_upgrade {
        Some(upgrade) => {
            let LastUpgrade { version, started_at, state } = upgrade;
            print!("Installation of version {version} was started at {started_at} ");
            match state {
                UpgradeState::InProgress => println!("and is {}", Color::Yellow.paint("still in progress")),
                UpgradeState::Success { finished_at } => {
                    println!("and was {} at {finished_at}", Color::Green.paint("completed"))
                }
                UpgradeState::Error { finished_at, error } => {
                    println!("and {} at {finished_at} with error: {error}", Color::Red.paint("failed"))
                }
            }
        }
        None => println!("No version installs in progress"),
    };
    Ok(())
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}

fn bytes_to_gb(bytes: u64) -> f64 {
    bytes_to_mb(bytes) as f64 / 1024.0
}

fn percent_to_color(percent: f64) -> Color {
    if percent < 0.4 {
        Color::Green
    } else if percent < 0.8 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn bool_to_color(value: bool) -> Color {
    match value {
        true => Color::Green,
        false => Color::Red,
    }
}

fn main() {
    let cli = Cli::parse();
    let Cli { url, api_key, command } = cli;

    let client = ApiClient::new(url, &api_key);
    let result = match command {
        Command::Launch(args) => launch(client, args),
        Command::List => list(client),
        Command::Delete(args) => delete(client, args),
        Command::Health(args) => health(client, args),
        Command::Start(args) => start(client, args),
        Command::Stop(args) => stop(client, args),
        Command::Restart(args) => restart(client, args),
        Command::Containers(command) => match command {
            ContainersCommand::List(args) => list_containers(client, args),
            ContainersCommand::Logs(args) => container_logs(client, args),
        },
        Command::System(command) => match command {
            SystemCommand::Logs(args) => system_logs(client, args),
            SystemCommand::Stats(args) => system_stats(client, args),
        },
        Command::Admin(AdminCommand::Artifacts(AdminArtifactsCommand::Install(args))) => {
            install_artifacts(client, args)
        }
        Command::Admin(AdminCommand::Artifacts(AdminArtifactsCommand::Version)) => artifacts_version(client),
        Command::Admin(AdminCommand::Artifacts(AdminArtifactsCommand::Cleanup)) => cleanup_artifacts(client),
        Command::Admin(AdminCommand::Agent(AdminAgentCommand::Upgrade(args))) => upgrade_agent(client, args),
        Command::Admin(AdminCommand::Agent(AdminAgentCommand::Version)) => agent_version(client),
    };
    if let Err(e) = result {
        eprintln!("Failed to run command: {e:#}");
        exit(1);
    }
}
