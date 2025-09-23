use crate::version::agent_version;
use crate::{config::ApiConfig, resources::SystemResources};
use anyhow::Context;
use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use chrono::{DateTime, Utc};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use strum::EnumDiscriminants;
use tracing::info;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait NilccApiClient: Send + Sync {
    /// Register an agent.
    async fn register(
        &self,
        config: &ApiConfig,
        resources: &SystemResources,
        public_ip: Ipv4Addr,
    ) -> Result<(), NilccApiError>;

    /// Report an event that occurred for a VM.
    async fn report_vm_event(
        &self,
        workload_id: Uuid,
        event: VmEvent,
        timestamp: DateTime<Utc>,
    ) -> Result<(), NilccApiError>;

    /// Send a heartbeat to the API.
    async fn heartbeat(&self, available_artifact_versions: Vec<String>) -> Result<HeartbeatResponse, NilccApiError>;
}

#[derive(Debug, Clone, Serialize, PartialEq, EnumDiscriminants)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum VmEvent {
    Starting,
    AwaitingCert,
    Running,
    Stopped,
    ForcedRestart,
    VmRestarted,
    FailedToStart { error: String },
    Warning { message: String },
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

    async fn send_request<T, R>(&self, method: Method, url: String, payload: &T) -> Result<R, NilccApiError>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let response = self.client.request(method, url).json(&payload).send().await?;
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let message = response.text().await.unwrap_or_default();
            Err(NilccApiError::Api { status, message })
        }
    }

    fn make_url(&self, endpoint: &str) -> String {
        let base = &self.api_base_url;
        format!("{base}{endpoint}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NilccApiError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: status={status}, message={message}")]
    Api { status: StatusCode, message: String },
}

#[async_trait]
impl NilccApiClient for HttpNilccApiClient {
    async fn register(
        &self,
        api_config: &ApiConfig,
        resources: &SystemResources,
        public_ip: Ipv4Addr,
    ) -> Result<(), NilccApiError> {
        let url = self.make_url("/api/v1/metal-instances/register");
        let payload = RegisterRequest {
            id: self.agent_id,
            agent_version: agent_version().to_string(),
            public_ip: public_ip.to_string(),
            token: api_config.token.clone(),
            hostname: resources.hostname.clone(),
            memory_mb: Resource { reserved: resources.reserved_memory_mb, total: resources.memory_mb },
            disk_space_gb: Resource { reserved: resources.reserved_disk_space_gb, total: resources.disk_space_gb },
            cpus: Resource { reserved: resources.reserved_cpus, total: resources.cpus },
            gpus: resources.gpus.as_ref().map(|g| g.addresses.len() as u32).unwrap_or_default(),
            gpu_model: resources.gpus.as_ref().map(|g| g.model.clone()),
        };
        self.send_request(Method::POST, url, &payload).await
    }

    async fn report_vm_event(
        &self,
        workload_id: Uuid,
        event: VmEvent,
        timestamp: DateTime<Utc>,
    ) -> Result<(), NilccApiError> {
        let url = self.make_url("/api/v1/workload-events/submit");
        let payload = VmEventRequest { agent_id: self.agent_id, workload_id, event, timestamp };
        self.send_request(Method::POST, url, &payload).await
    }

    async fn heartbeat(&self, available_artifact_versions: Vec<String>) -> Result<HeartbeatResponse, NilccApiError> {
        let url = self.make_url("/api/v1/metal-instances/heartbeat");
        let payload = HeartbeatRequest { id: self.agent_id, available_artifact_versions };
        self.send_request(Method::POST, url, &payload).await
    }
}

pub struct DummyNilccApiClient;

#[async_trait]
impl NilccApiClient for DummyNilccApiClient {
    async fn register(
        &self,
        _api_config: &ApiConfig,
        resources: &SystemResources,
        public_ip: Ipv4Addr,
    ) -> Result<(), NilccApiError> {
        info!("Registering with resources: {resources:?} and public IP: {public_ip}");
        Ok(())
    }

    async fn report_vm_event(
        &self,
        workload_id: Uuid,
        event: VmEvent,
        timestamp: DateTime<Utc>,
    ) -> Result<(), NilccApiError> {
        info!("Reporting VM event for {workload_id}: {event:?} @ {timestamp}");
        Ok(())
    }

    async fn heartbeat(&self, available_artifact_versions: Vec<String>) -> Result<HeartbeatResponse, NilccApiError> {
        info!("Reporting heartbeat, available versions = {available_artifact_versions:?}");
        Ok(HeartbeatResponse { expected_artifact_versions: available_artifact_versions })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    #[serde(rename = "metalInstanceId")]
    id: Uuid,
    agent_version: String,
    public_ip: String,
    token: String,
    hostname: String,
    memory_mb: Resource,
    disk_space_gb: Resource,
    cpus: Resource,
    gpus: u32,

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
    #[serde(rename = "metalInstanceId")]
    agent_id: Uuid,
    workload_id: Uuid,
    event: VmEvent,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HeartbeatRequest {
    #[serde(rename = "metalInstanceId")]
    id: Uuid,

    available_artifact_versions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponse {
    pub(crate) expected_artifact_versions: Vec<String>,
}
