use crate::funder::FunderHandle;
use alloy::primitives::{Address, Keccak256};
use nilcc_agent_models::{errors::RequestHandlerError, system::VerifierKey};
use reqwest::{
    Client, ClientBuilder, Response, StatusCode, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::de::DeserializeOwned;
use std::{collections::BTreeSet, time::Duration};
use tokio::time::{Interval, MissedTickBehavior, interval};
use tracing::{Instrument, error, info_span};

pub struct NilccAgentMonitorArgs {
    pub client: NilccAgentClient,
    pub poll_interval: Duration,
    pub funder_handle: FunderHandle,
}

pub struct NilccAgentMonitor {
    client: NilccAgentClient,
    ticker: Interval,
    funder_handle: FunderHandle,
    active_addresses: BTreeSet<Address>,
}

impl NilccAgentMonitor {
    pub fn spawn(args: NilccAgentMonitorArgs) {
        let NilccAgentMonitorArgs { client, poll_interval, funder_handle } = args;
        let mut ticker = interval(poll_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        let agent_url = client.base_url.host().map(|h| h.to_string()).unwrap_or_default();
        let monitor = Self { client, ticker, funder_handle, active_addresses: Default::default() };
        tokio::spawn(monitor.run().instrument(info_span!("agent", url = agent_url.to_string())));
    }

    async fn run(mut self) {
        loop {
            self.ticker.tick().await;

            if let Err(e) = self.run_once().await {
                error!("Failed to poll agent: {e}");
            }
        }
    }

    async fn run_once(&mut self) -> anyhow::Result<()> {
        let keys: Vec<VerifierKey> = self.client.get("api/v1/system/verifier/keys").await?;
        let active_addresses: BTreeSet<_> = keys
            .into_iter()
            .filter_map(|k| {
                k.active.then(|| {
                    let key = &k.public_key[1..];
                    let mut hasher = Keccak256::new();
                    hasher.update(key);
                    let digest = hasher.finalize();
                    let address = &digest[digest.len() - 20..];
                    Address::try_from(address).expect("invalid address")
                })
            })
            .collect();
        let removed_addresses: Vec<_> = self.active_addresses.difference(&active_addresses).collect();
        let new_addresses: Vec<_> = active_addresses.difference(&self.active_addresses).collect();
        for address in new_addresses {
            self.funder_handle.add_address(*address).await;
        }
        for address in removed_addresses {
            self.funder_handle.remove_address(*address).await;
        }
        self.active_addresses = active_addresses;
        Ok(())
    }
}

pub struct NilccAgentClient {
    base_url: Url,
    client: Client,
}

impl NilccAgentClient {
    pub fn new(base_url: Url, api_key: &str) -> Self {
        let mut headers = HeaderMap::new();
        let mut api_key = HeaderValue::from_str(&format!("Bearer {api_key}")).expect("invalid API key");
        api_key.set_sensitive(true);
        headers.insert(HeaderName::from_static("authorization"), api_key);

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
            Err(RequestError::Handler { code: err.error_code, details: err.message })
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
