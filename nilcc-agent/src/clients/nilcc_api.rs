use crate::models::{MetalInstance, SyncRequest, SyncResponse};
use anyhow::{bail, Context};
use async_trait::async_trait;
use reqwest::{Client, Method, RequestBuilder as ReqwestRequestBuilder, Response as ReqwestResponse};
use tracing::debug;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NilccApiClient: Send + Sync {
    /// Register an agent.
    async fn register(&self, payload: MetalInstance) -> anyhow::Result<()>;

    /// Reports the status of an agent
    async fn sync(&self, agent_id: Uuid, payload: SyncRequest) -> anyhow::Result<SyncResponse>;
}

pub struct HttpNilccApiClient {
    http_client: Client,
    api_base_url: String,
    api_key: String,
}

impl HttpNilccApiClient {
    /// Creates a new instance with the specified API base URL.
    pub fn new(api_base_url: String, api_key: String) -> anyhow::Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build reqwest client")?;
        debug!("AgentHttpRestClient initialized for base URL: {api_base_url}");
        Ok(Self { http_client, api_base_url, api_key })
    }

    fn prepare_request(&self, method: Method, endpoint_suffix: &str) -> ReqwestRequestBuilder {
        let url = format!("{}{endpoint_suffix}", self.api_base_url);
        self.http_client.request(method, &url).header("x-api-key", &self.api_key)
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        response: ReqwestResponse,
    ) -> anyhow::Result<T> {
        let status = response.status();
        let response_text = response.text().await.context(format!("Failed to read response text from {endpoint}"))?;

        if status.is_success() {
            serde_json::from_str(&response_text).with_context(|| {
                format!("Failed to deserialize successful response from {endpoint}. Status: {status}. Body: '{response_text}'")
            })
        } else {
            Err(anyhow::anyhow!("HTTP request to {endpoint} failed with status {status}: {response_text}"))
        }
    }
}

#[async_trait]
impl NilccApiClient for HttpNilccApiClient {
    async fn register(&self, payload: MetalInstance) -> anyhow::Result<()> {
        let endpoint_suffix = "/api/v1/metal-instances/~/register";
        let full_url = format!("{}{endpoint_suffix}", self.api_base_url,);
        debug!("Sending agent registration request to {full_url}: {payload:?}");

        let response = self
            .prepare_request(Method::POST, endpoint_suffix)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to send registration request to {full_url}"))?;

        if response.status().is_success() {
            Ok(())
        } else {
            bail!(
                "Failed to register agent at {full_url}. Status: {}; Error: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
    }

    async fn sync(&self, agent_id: Uuid, payload: SyncRequest) -> anyhow::Result<SyncResponse> {
        let endpoint_suffix = format!("/api/v1/metal-instances/{agent_id}/~/sync");
        let full_url = format!("{}{endpoint_suffix}", self.api_base_url);
        debug!("Sending agent sync request to {full_url}: {payload:?}");

        let response = self
            .prepare_request(Method::POST, &endpoint_suffix)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to send status report to {full_url}"))?;

        self.handle_response(&full_url, response).await
    }
}
