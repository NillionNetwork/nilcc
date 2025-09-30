use crate::api::{ApiClient, RequestError};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use uuid::Uuid;

mod api;
mod models;

#[derive(Parser)]
struct Cli {
    /// The endpoint where nilcc-api is reachable.
    #[clap(long, env = "NILCC_API_URL")]
    url: String,

    /// The API key to use.
    #[clap(long, env = "NILCC_API_KEY")]
    api_key: String,

    /// The command to execute.
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage accounts.
    #[clap(subcommand)]
    Accounts(AccountsCommand),

    /// Manage tiers.
    #[clap(subcommand)]
    Tiers(TiersCommand),

    /// Manage artifact versions.
    #[clap(subcommand)]
    Artifacts(ArtifactsCommand),

    /// Manage metal instances.
    #[clap(subcommand)]
    MetalInstances(MetalInstancesCommand),
}

#[derive(Subcommand)]
enum AccountsCommand {
    /// Create an account.
    Create(CreateAccountArgs),

    /// List the existing accounts.
    List,

    /// Add credits to an account.
    AddCredits(AddCreditsArgs),

    /// Rename an account.
    Rename(RenameAccountArgs),
}

#[derive(Args)]
struct CreateAccountArgs {
    /// The account name.
    name: String,

    /// The initial number of credits for this account.
    #[clap(default_value_t = 0)]
    credits: u64,
}

#[derive(Args)]
struct AddCreditsArgs {
    /// The account id.
    id: Uuid,

    /// The number of credits to grant to this account.
    credits: u64,
}

#[derive(Args)]
struct RenameAccountArgs {
    /// The account id.
    id: Uuid,

    /// The new account name.
    name: String,
}

#[derive(Subcommand)]
enum TiersCommand {
    /// Create a tier.
    Create(CreateTierArgs),

    /// Delete a tier.
    Delete {
        /// The identifier of the tier to be deleted.
        id: Uuid,
    },
}

#[derive(Args)]
struct CreateTierArgs {
    /// The tier name.
    name: String,

    /// The tier cost, expressed in credits/minute.
    #[clap(long)]
    cost: u64,

    /// The number of CPUs that are granted with this tier.
    #[clap(long)]
    cpus: u64,

    /// The number of GPUs that are granted with this tier.
    #[clap(long)]
    gpus: u64,

    /// The amount of memory in this tier, in MBs.
    #[clap(long)]
    memory_mb: u64,

    /// The amount of disk in this tier, in GBs.
    #[clap(long)]
    disk_gb: u64,
}

#[derive(Subcommand)]
enum ArtifactsCommand {
    /// Enable an artifact version.
    Enable {
        /// The version to enable.
        version: String,
    },

    /// Disable an artifact version.
    Disable {
        /// The version to disable.
        version: String,
    },
}

#[derive(Subcommand)]
enum MetalInstancesCommand {
    /// List all metal instances.
    List,

    /// Delete a metal instance.
    Delete {
        /// The identifier for the instance to delete.
        id: Uuid,
    },
}

struct Runner {
    client: ApiClient,
}

impl Runner {
    fn new(url: String, api_key: &str) -> Self {
        Self { client: ApiClient::new(url, api_key) }
    }

    fn create_account(&self, args: CreateAccountArgs) -> Result<serde_json::Value, RequestError> {
        let CreateAccountArgs { name, credits } = args;
        let request = models::accounts::CreateAccountRequest { name, credits };
        self.client.post("/api/v1/accounts/create", &request)
    }

    fn list_accounts(&self) -> Result<serde_json::Value, RequestError> {
        self.client.get("/api/v1/accounts/list")
    }

    fn add_credits(&self, args: AddCreditsArgs) -> Result<serde_json::Value, RequestError> {
        let AddCreditsArgs { id, credits } = args;
        let request = models::accounts::AddCreditsRequest { account_id: id, credits };
        self.client.post("/api/v1/accounts/add-credits", &request)
    }

    fn rename(&self, args: RenameAccountArgs) -> Result<serde_json::Value, RequestError> {
        let RenameAccountArgs { id, name } = args;
        let request = models::accounts::UpdateAccountRequest { account_id: id, name };
        self.client.post("/api/v1/accounts/update", &request)
    }

    fn create_tier(&self, args: CreateTierArgs) -> Result<serde_json::Value, RequestError> {
        let CreateTierArgs { name, cost, cpus, gpus, memory_mb, disk_gb } = args;
        let request = models::tiers::CreateTierRequest { name, cost, cpus, gpus, memory_mb, disk_gb };
        self.client.post("/api/v1/workload-tiers/create", &request)
    }

    fn delete_tier(&self, tier_id: Uuid) -> Result<serde_json::Value, RequestError> {
        let request = models::tiers::DeleteTierRequest { tier_id };
        self.client.post("/api/v1/workload-tiers/delete", &request)
    }

    fn enable_artifact_version(&self, version: String) -> Result<serde_json::Value, RequestError> {
        let request = models::artifacts::EnableArtifactVersionRequest { version };
        self.client.post("/api/v1/artifacts/enable", &request)
    }

    fn disable_artifact_version(&self, version: String) -> Result<serde_json::Value, RequestError> {
        let request = models::artifacts::DisableArtifactVersionRequest { version };
        self.client.post("/api/v1/artifacts/delete", &request)
    }

    fn list_metal_instances(&self) -> Result<serde_json::Value, RequestError> {
        self.client.get("/api/v1/metal-instances/list")
    }

    fn delete_metal_instance(&self, metal_instance_id: Uuid) -> Result<serde_json::Value, RequestError> {
        let request = models::metal_instances::DeleteMetalInstanceRequest { metal_instance_id };
        self.client.post("/api/v1/metal-instances/delete", &request)
    }
}

fn main() {
    let cli = Cli::parse();
    let runner = Runner::new(cli.url, &cli.api_key);
    let result = match cli.command {
        Command::Accounts(AccountsCommand::Create(args)) => runner.create_account(args),
        Command::Accounts(AccountsCommand::List) => runner.list_accounts(),
        Command::Accounts(AccountsCommand::AddCredits(args)) => runner.add_credits(args),
        Command::Accounts(AccountsCommand::Rename(args)) => runner.rename(args),
        Command::Tiers(TiersCommand::Create(args)) => runner.create_tier(args),
        Command::Tiers(TiersCommand::Delete { id }) => runner.delete_tier(id),
        Command::Artifacts(ArtifactsCommand::Enable { version }) => runner.enable_artifact_version(version),
        Command::Artifacts(ArtifactsCommand::Disable { version }) => runner.disable_artifact_version(version),
        Command::MetalInstances(MetalInstancesCommand::List) => runner.list_metal_instances(),
        Command::MetalInstances(MetalInstancesCommand::Delete { id }) => runner.delete_metal_instance(id),
    };
    let result = match result {
        Ok(response) => response,
        Err(e) => json!({"error": e.to_string()}),
    };
    let output = serde_json::to_string(&result).expect("failed to serialize");
    println!("{output}");
}
