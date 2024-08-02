use anyhow::Result;
use http::StatusCode;
use std::sync::{Arc, Mutex};
use url::Url;

use crate::pyoci::{AuthResponse, WwwAuth};
use crate::USER_AGENT;

/// HTTP Transport
///
/// This struct is responsible for sending HTTP requests to the upstream OCI registry.
#[derive(Debug, Default)]
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
    pub async fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let org_request = request.try_clone();
        let bearer_token = {
            // Local scope the bearer lock
            let token = self.bearer.lock().unwrap();
            token.clone()
        };
        let request = match bearer_token {
            Some(token) => request.header("Authorization", sens_header(&token)?),
            None => request,
        };
        let response = self._send(request).await?;
        if response.status() != StatusCode::UNAUTHORIZED {
            // No authentication needed or some error happened
            return Ok(response);
        }
        let Some(org_request) = org_request else {
            return Ok(response);
        };

        // Authenticate
        let www_auth: WwwAuth = match response.headers().get("WWW-Authenticate") {
            None => return Ok(response),
            Some(value) => match WwwAuth::parse(value.to_str()?) {
                Ok(value) => value,
                Err(_) => return Ok(response),
            },
        };
        let Some(basic_token) = &self.basic else {
            // No credentials provided
            return Ok(response);
        };

        let mut auth_url = Url::parse(&www_auth.realm)?;
        auth_url
            .query_pairs_mut()
            .append_pair("grant_type", "password")
            // if client_id is needed, add it here,
            // although GitHub does not seem to need a valid client_id
            // .append_pair("client_id", username)
            .append_pair("service", &www_auth.service);
        let auth_request = self.get(auth_url).header("Authorization", basic_token);
        let auth_response = self._send(auth_request).await?;

        if auth_response.status() != StatusCode::OK {
            // Authentication failed
            return Ok(auth_response);
        }

        let auth_response: AuthResponse = auth_response.json().await?;
        let bearer_token = {
            // Local scope the bearer lock and update the token
            let mut token = self.bearer.lock().unwrap();
            let new_token = format!("Bearer {}", auth_response.token);
            *token = Some(new_token.clone());
            new_token
        };
        self._send(org_request.header("Authorization", sens_header(&bearer_token)?))
            .await
    }

    /// Send a request
    async fn _send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let request = request.build()?;
        tracing::debug!("Request: {:#?}", request);
        let method = request.method().as_str().to_string();
        let url = request.url().to_owned().to_string();
        let response = self.client.execute(request).await?;
        let status: u16 = response.status().into();
        tracing::info!(method, status, url, "type" = "subrequest");
        tracing::debug!("Response Headers: {:#?}", response.headers());
        Ok(response)
    }

    /// Create a new GET request
    pub fn get(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.get(url)
    }
    /// Create a new POST request
    pub fn post(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.post(url)
    }
    /// Create a new PUT request
    pub fn put(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.put(url)
    }
    /// Create a new HEAD request
    pub fn head(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.head(url)
    }
}

/// Create a new HeaderValue with sensitive data
fn sens_header(value: &str) -> Result<reqwest::header::HeaderValue> {
    let mut header = reqwest::header::HeaderValue::from_str(value)?;
    header.set_sensitive(true);
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test happy-flow, no auth needed
    #[tokio::test]
    async fn http_transport_send() {
        let mut server = mockito::Server::new_async().await;
        let mocks = vec![
            server
                .mock("GET", "/foobar")
                .with_status(200)
                .with_body("Hello, world!")
                .create_async()
                .await,
        ];

        let transport = HttpTransport::new(None);
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, world!");
    }

    /// Test happy-flow, with authentication
    #[tokio::test]
    async fn http_transport_send_auth() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mocks = vec![
            // Response to unauthenticated request
            server
                .mock("GET", "/foobar")
                .with_status(401)
                .with_header(
                    "WWW-Authenticate",
                    &format!("Bearer realm=\"{url}/token\",service=\"pyoci.fakeservice\""),
                )
                .create_async()
                .await,
            // Token exchange
            server
                .mock(
                    "GET",
                    "/token?grant_type=password&service=pyoci.fakeservice",
                )
                .match_header("Authorization", "Basic mybasicauth")
                .with_status(200)
                .with_body(r#"{"token":"mytoken"}"#)
                .create_async()
                .await,
            // Re-submitted request, with bearer auth
            server
                .mock("GET", "/foobar")
                .match_header("Authorization", "Bearer mytoken")
                .with_status(200)
                .with_body("Hello, world!")
                .create_async()
                .await,
        ];

        let transport = HttpTransport::new(Some("Basic mybasicauth".to_string()));
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, world!");
    }

    /// Test missing authentication
    #[tokio::test]
    async fn http_transport_send_missing_auth() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mocks = vec![
            server
                .mock("GET", "/foobar")
                .with_status(401)
                .with_header(
                    "WWW-Authenticate",
                    &format!("Bearer realm=\"{url}/token\",service=\"pyoci.fakeservice\""),
                )
                .with_body("Unauthorized")
                .create_async()
                .await,
        ];

        let transport = HttpTransport::new(None);
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.text().await.unwrap(), "Unauthorized");
    }
    /// Test authentication failure
    #[tokio::test]
    async fn http_transport_send_auth_failure() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mocks = vec![
            server
                .mock("GET", "/foobar")
                .with_status(401)
                .with_header(
                    "WWW-Authenticate",
                    &format!("Bearer realm=\"{url}/token\",service=\"pyoci.fakeservice\""),
                )
                .with_body("Unauthorized")
                .create_async()
                .await,
            server
                .mock(
                    "GET",
                    "/token?grant_type=password&service=pyoci.fakeservice",
                )
                .with_status(418)
                .create_async()
                .await,
        ];

        let transport = HttpTransport::new(Some("Basic mybasicauth".to_string()));
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
        assert_eq!(response.text().await.unwrap(), "");
    }
    /// Test unauthorized
    #[tokio::test]
    async fn http_transport_send_unauthorized() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mocks = vec![
            server
                .mock("GET", "/foobar")
                .with_status(401)
                .with_header(
                    "WWW-Authenticate",
                    &format!("Bearer realm=\"{url}/token\",service=\"pyoci.fakeservice\""),
                )
                .with_body("Unauthorized")
                .create_async()
                .await,
            // Token exchange
            server
                .mock(
                    "GET",
                    "/token?grant_type=password&service=pyoci.fakeservice",
                )
                .match_header("Authorization", "Basic mybasicauth")
                .with_status(200)
                .with_body(r#"{"token":"mytoken"}"#)
                .create_async()
                .await,
            // Re-submitted request, with bearer auth
            server
                .mock("GET", "/foobar")
                .match_header("Authorization", "Bearer mytoken")
                .with_status(403)
                .with_body("Forbidden")
                .create_async()
                .await,
        ];

        let transport = HttpTransport::new(Some("Basic mybasicauth".to_string()));
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.text().await.unwrap(), "Forbidden");
    }
}
