use anyhow::Result;
use headers::authorization::Basic;
use headers::Authorization;
use std::future::poll_fn;
use tower::{Service, ServiceBuilder};

use crate::service::AuthLayer;
use crate::service::AuthService;
use crate::service::RequestLog;
use crate::service::RequestLogLayer;
use crate::USER_AGENT;

/// HTTP Transport
///
/// This struct is responsible for sending HTTP requests to the upstream OCI registry
/// while taking care of authentication.
#[derive(Debug, Clone)]
pub struct HttpTransport {
    client: reqwest::Client,
    service: AuthService<RequestLog<reqwest::Client>>,
}

impl HttpTransport {
    /// Create a new `HttpTransport`
    ///
    /// auth: Basic auth string
    ///       Will be swapped for a Bearer token if needed
    pub fn new(auth: Option<Authorization<Basic>>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap();
        Self {
            service: ServiceBuilder::new()
                .layer(AuthLayer::new(auth))
                .layer(RequestLogLayer::new("subrequest"))
                .service(client.clone()),
            client,
        }
    }

    /// Send a request
    ///
    /// When authentication is required, this method will automatically authenticate
    /// using the provided Basic auth string and caches the Bearer token for future requests within
    /// this session.
    pub async fn send(&mut self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let request = request.build()?;

        poll_fn(|ctx| self.service.poll_ready(ctx)).await?;
        let response = self.service.call(request).await?;

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
    /// Create a new DELETE request
    pub fn delete(&self, url: url::Url) -> reqwest::RequestBuilder {
        self.client.delete(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use url::Url;

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

        let mut transport = HttpTransport::new(None);
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
                .match_header("Authorization", "Basic dXNlcjpwYXNz")
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

        let mut transport = HttpTransport::new(Some(Authorization::basic("user", "pass")));
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, world!");
    }

    /// Test happy-flow, with authentication, multiple requests
    /// Subsequent requests should have their bearer token set without authenticating again
    #[tokio::test]
    async fn http_transport_send_auth_multiple_requests() {
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
                .match_header("Authorization", "Basic dXNlcjpwYXNz")
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
            // Second call to Send, should contain Bearer auth from last request
            server
                .mock("GET", "/bazqaz")
                .match_header("Authorization", "Bearer mytoken")
                .with_status(200)
                .with_body("Hello, again!")
                .create_async()
                .await,
        ];

        let mut transport = HttpTransport::new(Some(Authorization::basic("user", "pass")));
        // clone the transport to check if they share the bearer token state
        let mut transport2 = transport.clone();

        // First request, initiating authentication
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
        let response = transport.send(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, world!");

        // Second request, reusing the previous authentication
        let request = transport2.get(Url::parse(&format!("{url}/bazqaz")).unwrap());
        let response = transport2.send(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, again!");

        for mock in mocks {
            mock.assert_async().await;
        }
    }
    /// Test happy-flow, with anonymous authentication
    #[tokio::test]
    async fn http_transport_send_anonymous_auth() {
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
            // Anonymous token exchange
            server
                .mock(
                    "GET",
                    "/token?grant_type=password&service=pyoci.fakeservice",
                )
                .match_header("Authorization", mockito::Matcher::Missing)
                .with_body(r#"{"token":"anonymoustoken"}"#)
                .with_status(200)
                .create_async()
                .await,
            // Re-submitted request, with bearer auth
            server
                .mock("GET", "/foobar")
                .match_header("Authorization", "Bearer anonymoustoken")
                .with_status(200)
                .with_body("Hello, world!")
                .create_async()
                .await,
        ];

        let mut transport = HttpTransport::new(None);
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "Hello, world!");
    }
    /// Test missing authentication with anonymous token exchange denied
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
            // Anonymous token exchange denied
            server
                .mock(
                    "GET",
                    "/token?grant_type=password&service=pyoci.fakeservice",
                )
                .with_status(401)
                .with_body("Unauthorized")
                .create_async()
                .await,
        ];

        let mut transport = HttpTransport::new(None);
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
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

        let mut transport = HttpTransport::new(Some(Authorization::basic("user", "pass")));
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
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
                .match_header("Authorization", "Basic dXNlcjpwYXNz")
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

        let mut transport = HttpTransport::new(Some(Authorization::basic("user", "pass")));
        let request = transport.get(Url::parse(&format!("{url}/foobar")).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.text().await.unwrap(), "Forbidden");
    }
}
