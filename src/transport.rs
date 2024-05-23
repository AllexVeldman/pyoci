use std::sync::{Arc, Mutex};
use url::Url;

use crate::pyoci::{AuthResponse, WwwAuth};

static USER_AGENT: &str = concat!("pyoci ", env!("CARGO_PKG_VERSION"), " (cloudflare worker)");

/// HTTP Transport
///
/// This struct is responsible for sending HTTP requests to the upstream OCI registry.
#[derive(Default)]
pub struct HttpTransport {
    /// HTTP client
    client: reqwest::Client,
    /// Basic auth string, including the "Basic " prefix
    basic: Option<String>,
    /// Bearer token, including the "Bearer " prefix
    bearer: Arc<Mutex<Option<String>>>,
}

impl HttpTransport {
    /// Create a new HttpTransport
    ///
    /// auth: Basic auth string
    ///       Will be swapped for a Bearer token if needed
    pub fn new(auth: Option<String>) -> Self {
        let client = reqwest::Client::builder().user_agent(USER_AGENT);
        Self {
            client: client.build().unwrap(),
            basic: auth,
            bearer: Arc::new(Mutex::new(None)),
        }
    }

    /// Send a request
    ///
    /// When authentication is required, this method will automatically authenticate
    /// using the provided Basic auth string and caches the Bearer token for future requests within
    /// this session.
    pub async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let org_request = request.try_clone();
        let bearer_token = {
            // Local scope the bearer lock
            let token = self.bearer.lock().unwrap();
            token.clone()
        };
        let request = match bearer_token {
            Some(token) => request.header("Authorization", sens_header(&token)),
            None => request,
        };
        let response = self._send(request).await.expect("valid response");
        if response.status() != 401 {
            return Ok(response);
        }
        let Some(org_request) = org_request else {
            return Ok(response);
        };

        // Authenticate
        let www_auth: WwwAuth = match response.headers().get("WWW-Authenticate") {
            None => return Ok(response),
            Some(value) => match WwwAuth::parse(value.to_str().expect("valid header")) {
                Ok(value) => value,
                Err(_) => return Ok(response),
            },
        };
        let Some(basic_token) = &self.basic else {
            // No credentials provided
            return Ok(response);
        };

        let mut auth_url = Url::parse(&www_auth.realm).expect("valid url");
        auth_url
            .query_pairs_mut()
            .append_pair("grant_type", "password")
            // TODO: if client_id is needed, add it here
            // Although GitHub does not seem to need a valid client_id
            // .append_pair("client_id", username)
            .append_pair("service", &www_auth.service);
        let auth_request = self.get(auth_url).header("Authorization", basic_token);
        let auth_response = self._send(auth_request).await.expect("valid response");

        if auth_response.status() != 200 {
            return Ok(response);
        }

        let auth_response: AuthResponse = auth_response.json().await.expect("valid json");
        let bearer_token = {
            // Local scope the bearer lock and update the token
            let mut token = self.bearer.lock().unwrap();
            let new_token = format!("Bearer {}", auth_response.token);
            *token = Some(new_token.clone());
            new_token
        };
        self._send(org_request.header("Authorization", sens_header(&bearer_token)))
            .await
    }

    /// Send a request
    async fn _send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let request = request.build().expect("valid request");
        let method = request.method().as_str().to_string();
        let url = request.url().to_owned();
        let response = self.client.execute(request).await.expect("valid response");
        tracing::info!(
            "HTTP: [{method}] {status} {url}",
            method = method,
            status = response.status(),
            url = url
        );
        Ok(response)
    }

    /// Create a new GET request
    pub fn get(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.get(url)
    }
}

/// Create a new HeaderValue with sensitive data
fn sens_header(value: &str) -> reqwest::header::HeaderValue {
    let mut header = reqwest::header::HeaderValue::from_str(value).expect("valid header");
    header.set_sensitive(true);
    header
}
