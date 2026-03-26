use crate::api::{ApiClient, RequestError};
use clap::{Args, Parser, Subcommand, ValueEnum};
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
    #[clap(long, env = "NILCC_API_KEY", hide_env_values = true)]
    api_key: String,

    /// Print pretty JSON output.
    #[clap(short, long, global = true, env = "NILCC_PRETTY_PRINT")]
    pretty: bool,

    /// The command to execute.
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage accounts.
    #[clap(subcommand)]
    Accounts(AccountsCommand),

    /// Manage account API keys.
    #[clap(subcommand)]
    ApiKeys(ApiKeysCommand),

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

    /// Add USD balance to an account.
    AddBalance(AddBalanceArgs),

    /// Rename an account.
    Rename(RenameAccountArgs),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ApiKeyTypeArg {
    #[value(name = "account-admin")]
    AccountAdmin,
    #[value(name = "user")]
    User,
}

impl ApiKeyTypeArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::AccountAdmin => "account-admin",
            Self::User => "user",
        }
    }
}

#[derive(Subcommand)]
enum ApiKeysCommand {
    /// Create an API key for an account.
    Create(CreateApiKeyArgs),

    /// List the API keys for an account.
    List {
        /// The account id.
        account_id: Uuid,
    },

    /// Update an API key.
    Update(UpdateApiKeyArgs),

    /// Delete an API key.
    Delete {
        /// The identifier of the API key to be deleted.
        id: Uuid,
    },
}

#[derive(Args)]
struct CreateAccountArgs {
    /// The account name.
    name: String,

    /// The Ethereum wallet address for this account.
    wallet_address: String,

    /// The initial USD balance for this account.
    #[clap(long, default_value_t = 0.0)]
    balance: f64,
}

#[derive(Args)]
struct AddBalanceArgs {
    /// The account id.
    id: Uuid,

    /// The amount of USD to add to this account.
    balance: f64,
}

#[derive(Args)]
struct RenameAccountArgs {
    /// The account id.
    id: Uuid,

    /// The new account name.
    name: String,
}

#[derive(Args)]
struct CreateApiKeyArgs {
    /// The account id.
    account_id: Uuid,

    /// The type of API key to create.
    #[clap(long, value_enum)]
    key_type: ApiKeyTypeArg,

    /// Create the key as inactive.
    #[clap(long, default_value_t = false)]
    inactive: bool,
}

#[derive(Args)]
struct UpdateApiKeyArgs {
    /// The identifier of the API key to update.
    id: Uuid,

    /// Set the API key type.
    #[clap(long, value_enum)]
    key_type: Option<ApiKeyTypeArg>,

    /// Set whether the API key is active.
    #[clap(long)]
    active: Option<bool>,
}

#[derive(Subcommand)]
enum TiersCommand {
    /// Create a tier.
    Create(CreateTierArgs),

    /// List tiers.
    List,

    /// Update a tier.
    Update(UpdateTierArgs),

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

    /// The tier cost in USD/minute.
    #[clap(long)]
    cost: f64,

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

#[derive(Args)]
struct UpdateTierArgs {
    /// The identifier of the tier to be updated.
    id: Uuid,

    /// The tier name.
    name: String,

    /// The tier cost in USD/minute.
    #[clap(long)]
    cost: f64,

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

    /// List artifact versions.
    List,

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
        let CreateAccountArgs { name, wallet_address, balance } = args;
        let request = models::accounts::CreateAccountRequest { name, wallet_address, balance };
        self.client.post("/api/v1/accounts/create", &request)
    }

    fn list_accounts(&self) -> Result<serde_json::Value, RequestError> {
        self.client.get("/api/v1/accounts/list")
    }

    fn add_balance(&self, args: AddBalanceArgs) -> Result<serde_json::Value, RequestError> {
        let AddBalanceArgs { id, balance } = args;
        let request = models::accounts::AddBalanceRequest { account_id: id, balance };
        self.client.post("/api/v1/accounts/add-balance", &request)
    }

    fn rename(&self, args: RenameAccountArgs) -> Result<serde_json::Value, RequestError> {
        let RenameAccountArgs { id, name } = args;
        let request = models::accounts::UpdateAccountRequest { account_id: id, name };
        self.client.post("/api/v1/accounts/update", &request)
    }

    fn create_api_key(&self, args: CreateApiKeyArgs) -> Result<serde_json::Value, RequestError> {
        let CreateApiKeyArgs { account_id, key_type, inactive } = args;
        let request = models::api_keys::CreateApiKeyRequest {
            account_id,
            r#type: key_type.as_str().to_string(),
            active: !inactive,
        };
        self.client.post("/api/v1/api-keys/create", &request)
    }

    fn list_api_keys(&self, account_id: Uuid) -> Result<serde_json::Value, RequestError> {
        self.client.get(&format!("/api/v1/api-keys/account/{account_id}"))
    }

    fn update_api_key(&self, args: UpdateApiKeyArgs) -> Result<serde_json::Value, RequestError> {
        let UpdateApiKeyArgs { id, key_type, active } = args;
        let request = models::api_keys::UpdateApiKeyRequest {
            id,
            r#type: key_type.map(|value| value.as_str().to_string()),
            active,
        };
        self.client.put("/api/v1/api-keys/update", &request)
    }

    fn delete_api_key(&self, id: Uuid) -> Result<serde_json::Value, RequestError> {
        let request = models::api_keys::DeleteApiKeyRequest { id };
        self.client.post("/api/v1/api-keys/delete", &request)
    }

    fn create_tier(&self, args: CreateTierArgs) -> Result<serde_json::Value, RequestError> {
        let CreateTierArgs { name, cost, cpus, gpus, memory_mb, disk_gb } = args;
        let request = models::tiers::CreateTierRequest { name, cost, cpus, gpus, memory_mb, disk_gb };
        self.client.post("/api/v1/workload-tiers/create", &request)
    }

    fn list_tiers(&self) -> Result<serde_json::Value, RequestError> {
        self.client.get("/api/v1/workload-tiers/list")
    }

    fn update_tier(&self, args: UpdateTierArgs) -> Result<serde_json::Value, RequestError> {
        let UpdateTierArgs { id, name, cost, cpus, gpus, memory_mb, disk_gb } = args;
        let request = models::tiers::UpdateTierRequest { tier_id: id, name, cost, cpus, gpus, memory_mb, disk_gb };
        self.client.put("/api/v1/workload-tiers/update", &request)
    }

    fn delete_tier(&self, tier_id: Uuid) -> Result<serde_json::Value, RequestError> {
        let request = models::tiers::DeleteTierRequest { tier_id };
        self.client.post("/api/v1/workload-tiers/delete", &request)
    }

    fn enable_artifact_version(&self, version: String) -> Result<serde_json::Value, RequestError> {
        let request = models::artifacts::EnableArtifactVersionRequest { version };
        self.client.post("/api/v1/artifacts/enable", &request)
    }

    fn list_artifact_versions(&self) -> Result<serde_json::Value, RequestError> {
        self.client.get("/api/v1/artifacts/list")
    }

    fn disable_artifact_version(&self, version: String) -> Result<serde_json::Value, RequestError> {
        let request = models::artifacts::DisableArtifactVersionRequest { version };
        self.client.post("/api/v1/artifacts/disable", &request)
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
        Command::Accounts(AccountsCommand::AddBalance(args)) => runner.add_balance(args),
        Command::Accounts(AccountsCommand::Rename(args)) => runner.rename(args),
        Command::ApiKeys(ApiKeysCommand::Create(args)) => runner.create_api_key(args),
        Command::ApiKeys(ApiKeysCommand::List { account_id }) => runner.list_api_keys(account_id),
        Command::ApiKeys(ApiKeysCommand::Update(args)) => runner.update_api_key(args),
        Command::ApiKeys(ApiKeysCommand::Delete { id }) => runner.delete_api_key(id),
        Command::Tiers(TiersCommand::Create(args)) => runner.create_tier(args),
        Command::Tiers(TiersCommand::List) => runner.list_tiers(),
        Command::Tiers(TiersCommand::Update(args)) => runner.update_tier(args),
        Command::Tiers(TiersCommand::Delete { id }) => runner.delete_tier(id),
        Command::Artifacts(ArtifactsCommand::Enable { version }) => runner.enable_artifact_version(version),
        Command::Artifacts(ArtifactsCommand::List) => runner.list_artifact_versions(),
        Command::Artifacts(ArtifactsCommand::Disable { version }) => runner.disable_artifact_version(version),
        Command::MetalInstances(MetalInstancesCommand::List) => runner.list_metal_instances(),
        Command::MetalInstances(MetalInstancesCommand::Delete { id }) => runner.delete_metal_instance(id),
    };
    let result = match result {
        Ok(response) => response,
        Err(e) => json!({"error": e.to_string()}),
    };
    let output = match cli.pretty {
        true => serde_json::to_string_pretty(&result),
        false => serde_json::to_string(&result),
    }
    .expect("failed to serialize");
    println!("{output}");
}
