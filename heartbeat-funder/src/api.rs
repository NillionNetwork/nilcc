use reqwest::{
    Client, ClientBuilder, Response, StatusCode, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::BTreeSet, time::Duration};
use tokio::time::{Interval, MissedTickBehavior, interval};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    agent::{NilccAgentClient, NilccAgentMonitor, NilccAgentMonitorArgs},
    funder::FunderHandle,
};

pub struct NilccApiMonitorArgs {
    pub client: NilccApiClient,
    pub poll_interval: Duration,
    pub agent_poll_interval: Duration,
    pub funder_handle: FunderHandle,
}

pub struct NilccApiMonitor {
    client: NilccApiClient,
    ticker: Interval,
    agent_poll_interval: Duration,
    funder_handle: FunderHandle,
    agents: BTreeSet<Uuid>,
}

impl NilccApiMonitor {
    pub fn spawn(args: NilccApiMonitorArgs) {
        let NilccApiMonitorArgs { client, poll_interval, agent_poll_interval, funder_handle } = args;
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let monitor =
            NilccApiMonitor { client, ticker, funder_handle, agent_poll_interval, agents: Default::default() };
        tokio::spawn(monitor.run());
    }

    async fn run(mut self) {
        info!("Starting nilcc-api monitor using URL {}", self.client.base_url);
        loop {
            self.ticker.tick().await;

            if let Err(e) = self.run_once().await {
                error!("Failed to poll api: {e}");
            }
        }
    }

    async fn run_once(&mut self) -> anyhow::Result<()> {
        let instances: Vec<MetalInstance> = self.client.get("api/v1/metal-instances/list").await?;
        for instance in instances {
            let MetalInstance { id, domain, token } = instance;
            if self.agents.contains(&id) {
                continue;
            }
            let url = match format!("https://{domain}").parse() {
                Ok(url) => url,
                Err(e) => {
                    error!("Invalid agent hostname {domain} found in nilcc-api: {e}");
                    continue;
                }
            };
            info!("Starting to monitor new agent {id} at {url}");

            let client = NilccAgentClient::new(url, &token);
            NilccAgentMonitor::spawn(NilccAgentMonitorArgs {
                client,
                poll_interval: self.agent_poll_interval,
                funder_handle: self.funder_handle.clone(),
                name: None,
            });
            self.agents.insert(instance.id);
        }
        Ok(())
    }
}

pub struct NilccApiClient {
    base_url: Url,
    client: Client,
}

impl NilccApiClient {
    pub fn new(base_url: Url, api_key: &str) -> Self {
        let mut headers = HeaderMap::new();
        let mut api_key = HeaderValue::from_str(api_key).expect("invalid API key");
        api_key.set_sensitive(true);
        headers.insert(HeaderName::from_static("x-api-key"), api_key);

        let client = ClientBuilder::new().default_headers(headers).build().expect("failed to build client");
        Self { base_url, client }
    }

    async fn get<O>(&self, path: &str) -> Result<O, RequestError>
    where
        O: DeserializeOwned,
    {
        let url = self.make_url(path);
        let response = self.client.get(url).send().await?;
        Self::handle_response(response).await
    }

    async fn handle_response<O>(response: Response) -> Result<O, RequestError>
    where
        O: DeserializeOwned,
    {
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let err: RequestHandlerError = response.json().await.map_err(|_| RequestError::InvalidError(status))?;
            Err(RequestError::Handler { code: err.kind, details: err.error })
        }
    }

    fn make_url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("sending request: {0}")]
    Request(#[from] reqwest::Error),

    #[error("api error, code = {code}, details = {details}")]
    Handler { code: String, details: String },

    #[error("invalid error response for status: {0}")]
    InvalidError(StatusCode),
}

#[derive(Deserialize)]
struct RequestHandlerError {
    kind: String,
    error: String,
}

#[derive(Deserialize)]
struct MetalInstance {
    #[serde(rename = "metalInstanceId")]
    id: Uuid,
    domain: String,
    token: String,
}
