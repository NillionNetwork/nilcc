use crate::resources::SystemResources;
use crate::version::agent_version;
use anyhow::{bail, Context};
use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method};
use serde::Serialize;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NilccApiClient: Send + Sync {
    /// Register an agent.
    async fn register(&self, resources: &SystemResources) -> anyhow::Result<()>;

    /// Report an event that occurred for a VM.
    async fn report_vm_event(&self, workload_id: Uuid, event: VmEvent) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VmEvent {
    Started,
    Stopped,
    FailedToStart { error: String },
}

pub struct NilccApiClientArgs {
    pub api_base_url: String,
    pub api_key: String,
    pub agent_id: Uuid,
}

pub struct HttpNilccApiClient {
    client: Client,
    api_base_url: String,
    agent_id: Uuid,
}

impl HttpNilccApiClient {
    /// Creates a new instance with the specified API base URL.
    pub fn new(args: NilccApiClientArgs) -> anyhow::Result<Self> {
        let NilccApiClientArgs { api_base_url, api_key, agent_id } = args;
        let mut headers = HeaderMap::new();
        let mut api_key = HeaderValue::from_str(&api_key).context("Invalid API key")?;
        api_key.set_sensitive(true);
        headers.insert(HeaderName::from_static("x-api-key"), api_key);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .context("Failed to build reqwest client")?;
        Ok(Self { client, api_base_url, agent_id })
    }

    async fn send_request<T>(&self, method: Method, url: String, payload: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let response = self
            .client
            .request(method, url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send registration request")?;

        if response.status().is_success() {
            Ok(())
        } else {
            bail!("Request failed: status={}, error={}", response.status(), response.text().await.unwrap_or_default());
        }
    }

    fn make_url(&self, endpoint: &str) -> String {
        let base = &self.api_base_url;
        format!("{base}{endpoint}")
    }
}

#[async_trait]
impl NilccApiClient for HttpNilccApiClient {
    async fn register(&self, resources: &SystemResources) -> anyhow::Result<()> {
        let url = self.make_url("/api/v1/metal-instances/~/register");
        let payload = RegisterRequest {
            id: self.agent_id,
            agent_version: agent_version().to_string(),
            hostname: resources.hostname.clone(),
            memory_mb: Resource { reserved: resources.reserved_memory_mb, total: resources.memory_mb },
            disk_space_gb: Resource { reserved: resources.reserved_disk_space_gb, total: resources.disk_space_gb },
            cpus: Resource { reserved: resources.reserved_cpus, total: resources.cpus },
            gpus: resources.gpus.as_ref().map(|g| g.addresses.len() as u32),
            gpu_model: resources.gpus.as_ref().map(|g| g.model.clone()),
        };
        self.send_request(Method::POST, url, &payload).await.context("Failed to register agent")
    }

    async fn report_vm_event(&self, workload_id: Uuid, event: VmEvent) -> anyhow::Result<()> {
        let url = self.make_url("/api/v1/metal-instances/~/events/submit");
        let payload = VmEventRequest { agent_id: self.agent_id, workload_id, event };
        self.send_request(Method::POST, url, &payload).await.context("Failed to submit event")
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    id: Uuid,
    agent_version: String,
    hostname: String,
    memory_mb: Resource,
    disk_space_gb: Resource,
    cpus: Resource,

    #[serde(skip_serializing_if = "Option::is_none")]
    gpus: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    gpu_model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Resource {
    reserved: u32,
    total: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VmEventRequest {
    agent_id: Uuid,
    workload_id: Uuid,
    event: VmEvent,
}
