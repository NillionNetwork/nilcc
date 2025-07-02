use crate::models::{MetalInstance, MetalInstanceDetails};
use crate::resources::SystemResources;
use crate::version::agent_version;
use anyhow::{bail, Context};
use async_trait::async_trait;
use reqwest::{Client, Method, RequestBuilder as ReqwestRequestBuilder};
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NilccApiClient: Send + Sync {
    /// Register an agent.
    async fn register(&self, resources: &SystemResources) -> anyhow::Result<()>;
}

pub struct NilccApiClientArgs {
    pub api_base_url: String,
    pub api_key: String,
    pub agent_id: Uuid,
}

pub struct HttpNilccApiClient {
    http_client: Client,
    api_base_url: String,
    api_key: String,
    agent_id: Uuid,
}

impl HttpNilccApiClient {
    /// Creates a new instance with the specified API base URL.
    pub fn new(args: NilccApiClientArgs) -> anyhow::Result<Self> {
        let NilccApiClientArgs { api_base_url, api_key, agent_id } = args;
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build reqwest client")?;
        Ok(Self { http_client, api_base_url, api_key, agent_id })
    }

    fn prepare_request(&self, method: Method, url: String) -> ReqwestRequestBuilder {
        self.http_client.request(method, &url).header("x-api-key", &self.api_key)
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
        let payload = MetalInstance {
            id: self.agent_id,
            details: MetalInstanceDetails {
                agent_version: agent_version().to_string(),
                hostname: resources.hostname.clone(),
                memory_gb: resources.memory_gb,
                os_reserved_memory_gb: resources.reserved_memory_gb,
                disk_space_gb: resources.disk_space_gb,
                os_reserved_disk_space_gb: resources.reserved_disk_space_gb,
                cpus: resources.cpus,
                os_reserved_cpus: resources.reserved_cpus,
                gpus: resources.gpus.as_ref().map(|g| g.addresses.len() as u32),
                gpu_model: resources.gpus.as_ref().map(|g| g.model.clone()),
            },
        };

        let response = self
            .prepare_request(Method::POST, url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send registration request")?;

        if response.status().is_success() {
            Ok(())
        } else {
            bail!(
                "Failed to register agent: status={}, error={}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
    }
}
