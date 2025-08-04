use anyhow::Context;
use async_trait::async_trait;
use cvm_agent_models::{
    bootstrap::BootstrapRequest,
    container::Container,
    logs::{ContainerLogsRequest, ContainerLogsResponse},
};
use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use tracing::info;

#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait CvmAgentClient: Send + Sync {
    async fn list_containers(&self, cvm_agent_port: u16) -> Result<Vec<Container>, CvmAgentRequestError>;
    async fn logs(
        &self,
        cvm_agent_port: u16,
        request: &ContainerLogsRequest,
    ) -> Result<ContainerLogsResponse, CvmAgentRequestError>;
    async fn check_health(&self, cvm_agent_port: u16) -> Result<(), CvmAgentRequestError>;
    async fn bootstrap(&self, cvm_agent_port: u16, request: &BootstrapRequest) -> Result<(), CvmAgentRequestError>;
}

pub struct DefaultCvmAgentClient {
    client: Client,
}

impl DefaultCvmAgentClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to build reqwest client")?;
        Ok(Self { client })
    }

    async fn get<Q: Serialize, T: DeserializeOwned>(
        &self,
        port: u16,
        path: &str,
        query: &Q,
    ) -> Result<T, CvmAgentRequestError> {
        let endpoint = format!("http://127.0.0.1:{port}{path}");
        info!("Sending request to {endpoint}");
        let response = self.client.get(endpoint).query(query).send().await?.error_for_status()?.json().await?;
        Ok(response)
    }

    async fn post<R: Serialize>(&self, port: u16, path: &str, request: &R) -> Result<(), CvmAgentRequestError> {
        let endpoint = format!("http://127.0.0.1:{port}{path}");
        info!("Sending request to {endpoint}");
        self.client.post(endpoint).json(request).send().await?.error_for_status()?;
        Ok(())
    }
}

#[async_trait]
impl CvmAgentClient for DefaultCvmAgentClient {
    async fn list_containers(&self, cvm_agent_port: u16) -> Result<Vec<Container>, CvmAgentRequestError> {
        self.get(cvm_agent_port, "/api/v1/containers/list", &()).await
    }

    async fn logs(
        &self,
        cvm_agent_port: u16,
        request: &ContainerLogsRequest,
    ) -> Result<ContainerLogsResponse, CvmAgentRequestError> {
        self.get(cvm_agent_port, "/api/v1/containers/logs", &request).await
    }

    async fn check_health(&self, cvm_agent_port: u16) -> Result<(), CvmAgentRequestError> {
        self.get(cvm_agent_port, "/api/v1/health", &()).await
    }

    async fn bootstrap(&self, cvm_agent_port: u16, request: &BootstrapRequest) -> Result<(), CvmAgentRequestError> {
        self.post(cvm_agent_port, "/api/v1/system/bootstrap", request).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CvmAgentRequestError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("response decode: {0}")]
    Decode(#[from] serde_json::Error),
}
