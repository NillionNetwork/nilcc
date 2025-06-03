use crate::data_schemas::{MetalInstanceDetails, RegistrationResponse, SyncRequest, SyncResponse};
use anyhow::Context;
use reqwest::{Client, Method, RequestBuilder as ReqwestRequestBuilder, Response as ReqwestResponse};
use tracing::debug;
use uuid::Uuid;

pub struct AgentHttpRestClient {
    http_client: Client,
    api_base_url: String,
    api_key: Option<String>,
}

impl AgentHttpRestClient {
    /// Creates a new instance with the specified API base URL.
    pub fn new(api_base_url: String, api_key: Option<String>) -> anyhow::Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to build reqwest client")?;
        debug!("AgentHttpRestClient initialized for base URL: {}", api_base_url);
        Ok(Self { http_client, api_base_url, api_key })
    }

    fn prepare_request(&self, method: Method, endpoint_suffix: &str) -> ReqwestRequestBuilder {
        let url = format!("{}{}", self.api_base_url, endpoint_suffix);
        let mut request_builder = self.http_client.request(method, &url);
        if let Some(key) = &self.api_key {
            request_builder = request_builder.header("x-api-key", key);
        }
        request_builder
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        response: ReqwestResponse,
    ) -> anyhow::Result<T> {
        let status = response.status();
        let response_text = response.text().await.context(format!("Failed to read response text from {}", endpoint))?;

        if status.is_success() {
            serde_json::from_str(&response_text).with_context(|| {
                format!(
                    "Failed to deserialize successful response from {}. Status: {}. Body: '{}'",
                    endpoint, status, response_text
                )
            })
        } else {
            Err(anyhow::anyhow!("HTTP request to {} failed with status {}: {}", endpoint, status, response_text))
        }
    }

    /// Registers an agent
    pub async fn register(&self, payload: MetalInstanceDetails) -> anyhow::Result<RegistrationResponse> {
        let endpoint_suffix = "/api/v1/metal-instances/~/register";
        let full_url = format!("{}{}", self.api_base_url, endpoint_suffix);
        debug!("Sending agent registration request to {}: {:?}", full_url, payload);

        let response = self
            .prepare_request(Method::POST, endpoint_suffix)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to send registration request to {}", full_url))?;

        self.handle_response(&full_url, response).await
    }

    /// Reports the status of an agent
    pub async fn sync(&self, agent_id: Uuid, payload: SyncRequest) -> anyhow::Result<SyncResponse> {
        let endpoint_suffix = format!("/api/v1/metal-instances/{}/~/sync", agent_id);
        let full_url = format!("{}{}", self.api_base_url, endpoint_suffix);
        debug!("Sending agent sync request to {}: {:?}", full_url, payload);

        let response = self
            .prepare_request(Method::POST, &endpoint_suffix)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to send status report to {}", full_url))?;

        self.handle_response(&full_url, response).await
    }
}
