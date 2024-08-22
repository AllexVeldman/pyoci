use anyhow::Result;
use std::boxed::Box;
use std::future::poll_fn;
use std::future::Future;
use std::pin::Pin;
use tower::{Service, ServiceBuilder};

use crate::service::AuthLayer;
use crate::service::RequestLogLayer;
use crate::USER_AGENT;

/// HTTP Transport
///
/// This struct is responsible for sending HTTP requests to the upstream OCI registry.
#[derive(Debug, Default, Clone)]
pub struct HttpTransport {
    /// HTTP client
    client: reqwest::Client,
    /// Authentication layer
    auth_layer: AuthLayer,
}

// Wraps the reqwest client so we can implement Service.
// reqwest implements Service normally but not for the WASM target.
// This allows us to use other Service implementations to wrap the reqwest client.
impl Service<reqwest::Request> for HttpTransport {
    type Response = reqwest::Response;
    type Error = reqwest::Error;
    // we need to box the future as we currently can't express the anonymous `impl Future` type
    // returned by reqwest::Client::execute
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: reqwest::Request) -> Self::Future {
        #[cfg(target_arch = "wasm32")]
        let fut = Box::pin(worker::send::SendFuture::new(self.client.execute(request)));
        #[cfg(not(target_arch = "wasm32"))]
        let fut = Box::pin(self.client.execute(request));
        fut
    }
}

impl HttpTransport {
    /// Create a new HttpTransport
    ///
    /// auth: Basic auth string
    ///       Will be swapped for a Bearer token if needed
    pub fn new(auth: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder().user_agent(USER_AGENT);
        Ok(Self {
            client: client.build()?,
            auth_layer: AuthLayer::new(auth)?,
        })
    }

    /// Send a request
    ///
    /// When authentication is required, this method will automatically authenticate
    /// using the provided Basic auth string and caches the Bearer token for future requests within
    /// this session.
    pub async fn send(&mut self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let request = request.build()?;
        tracing::debug!("Request: {:#?}", request);

        let mut service = ServiceBuilder::new()
            .layer(self.auth_layer.clone())
            .layer(RequestLogLayer::new("subrequest"))
            .service(self.clone());
        poll_fn(|ctx| service.poll_ready(ctx)).await?;
        let response = service.call(request).await?;

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

        let mut transport = HttpTransport::new(None).unwrap();
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

        let mut transport = HttpTransport::new(Some("Basic mybasicauth".to_string())).unwrap();
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

        let mut transport = HttpTransport::new(None).unwrap();
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

        let mut transport = HttpTransport::new(Some("Basic mybasicauth".to_string())).unwrap();
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

        let mut transport = HttpTransport::new(Some("Basic mybasicauth".to_string())).unwrap();
        let request = transport.get(Url::parse(&format!("{}/foobar", &server.url())).unwrap());
        let response = transport.send(request).await.unwrap();
        for mock in mocks {
            mock.assert_async().await;
        }
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.text().await.unwrap(), "Forbidden");
    }
}
