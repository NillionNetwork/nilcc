use nilcc_agent_models::errors::RequestHandlerError;
use reqwest::{
    StatusCode,
    blocking::{Client, ClientBuilder, Response},
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::{Serialize, de::DeserializeOwned};

pub struct ApiClient {
    base_url: String,
    client: Client,
}

impl ApiClient {
    pub fn new(base_url: String, api_key: &str) -> Self {
        let mut headers = HeaderMap::new();
        let mut api_key = HeaderValue::from_str(&format!("Bearer {api_key}")).expect("invalid API key");
        api_key.set_sensitive(true);
        headers.insert(HeaderName::from_static("authorization"), api_key);

        let client = ClientBuilder::new().default_headers(headers).build().expect("failed to build client");
        Self { base_url, client }
    }

    pub fn post<T, O>(&self, path: &str, request: &T) -> Result<O, RequestError>
    where
        T: Serialize,
        O: DeserializeOwned,
    {
        let url = self.make_url(path);
        let response = self.client.post(url).json(request).send()?;
        Self::handle_response(response)
    }

    pub fn get<O>(&self, path: &str) -> Result<O, RequestError>
    where
        O: DeserializeOwned,
    {
        let url = self.make_url(path);
        let response = self.client.get(url).send()?;
        Self::handle_response(response)
    }

    pub fn get_query<T, O>(&self, path: &str, query: &T) -> Result<O, RequestError>
    where
        T: Serialize,
        O: DeserializeOwned,
    {
        let url = self.make_url(path);
        let response = self.client.get(url).query(query).send()?;
        Self::handle_response(response)
    }

    fn handle_response<O>(response: Response) -> Result<O, RequestError>
    where
        O: DeserializeOwned,
    {
        if response.status().is_success() {
            Ok(response.json()?)
        } else {
            let status = response.status();
            let err: RequestHandlerError = response.json().map_err(|_| RequestError::InvalidError(status))?;
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
