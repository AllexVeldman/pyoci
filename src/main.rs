#![warn(unused_extern_crates)]

// Webserver request handlers
mod app;
// OTLP handlers
#[cfg(feature = "otlp")]
mod otlp;
// Helper for parsing and managing Python/OCI packages
mod package;
// PyOci client
mod pyoci;
// Askama templates
mod templates;
// HTTP Transport
mod transport;
// Services
mod service;

// Test mocks
#[cfg(test)]
mod mocks;

pub use pyoci::PyOci;
use tokio::task::JoinHandle;

use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::Subscriber;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::app::router;

#[cfg(feature = "otlp")]
use crate::otlp::otlp;

// crate constants
const PYOCI_VERSION: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT: &str = concat!("pyoci ", env!("CARGO_PKG_VERSION"));
const ARTIFACT_TYPE: &str = "application/pyoci.package.v1";

/// Runtime environment variables
#[derive(Debug, Clone)]
struct Env {
    port: u16,
    rust_log: String,
    path: Option<String>,
    otlp_endpoint: Option<String>,
    otlp_auth: Option<String>,
    deployment_env: Option<String>,
    container_name: Option<String>,
    pod_name: Option<String>,
    replica_name: Option<String>,
}

impl Env {
    #[cfg(test)]
    fn default() -> Self {
        Self {
            port: 8080,
            rust_log: "info".to_string(),
            path: None,
            otlp_endpoint: None,
            otlp_auth: None,
            deployment_env: None,
            container_name: None,
            pod_name: None,
            replica_name: None,
        }
    }
    fn new() -> Self {
        Self {
            port: env::var("PORT")
                .unwrap_or("8080".to_string())
                .parse()
                .expect("Failed to parse PORT"),
            rust_log: env::var("RUST_LOG").unwrap_or("info".to_string()),
            path: env::var("PYOCI_PATH").ok(),
            otlp_endpoint: env::var("OTLP_ENDPOINT").ok(),
            otlp_auth: env::var("OTLP_AUTH").ok(),
            deployment_env: env::var("DEPLOYMENT_ENVIRONMENT").ok(),
            // https://learn.microsoft.com/en-us/azure/container-apps/environment-variables
            container_name: env::var("CONTAINER_APP_NAME").ok(),
            pod_name: env::var("CONTAINER_APP_REVISION").ok(),
            replica_name: env::var("CONTAINER_APP_REPLICA_NAME").ok(),
        }
    }

    fn trace_attributes(&self) -> HashMap<&'static str, Option<String>> {
        HashMap::from([
            ("service.name", Some("pyoci".to_string())),
            ("service.version", Some(PYOCI_VERSION.to_string())),
            ("deployment.environment", self.deployment_env.clone()),
            ("k8s.container.name", self.container_name.clone()),
            ("k8s.pod.name", self.pod_name.clone()),
            ("k8s.replicaset.name", self.replica_name.clone()),
        ])
    }
}

#[tokio::main]
async fn main() {
    let environ = Env::new();
    let cancel_token = CancellationToken::new();
    let (tracing, otlp_handle) = setup_tracing(&environ, cancel_token.clone());
    tracing.init();
    tracing::info!("Tracing initialized");
    if otlp_handle.is_some() {
        tracing::info!("Sending logs/traces to OTLP collector");
    }

    // Setup the webserver
    tracing::info!(
        "Listening on 0.0.0.0:{}{}",
        environ.port,
        &environ.path.clone().unwrap_or("".to_string())
    );
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", environ.port))
        .await
        .unwrap();
    axum::serve(listener, router(environ.path))
        .with_graceful_shutdown(shutdown_signal(cancel_token, otlp_handle))
        .await
        .unwrap();
}

/// Setup tracing with a console log and OTLP trace/log if the `otlp` feature is enabled.
/// If the JoinHandle is not None, ensure to await it before shutting down to send the remaining
/// trace data to the OTLP collector
fn setup_tracing(
    environ: &Env,
    cancel_token: CancellationToken,
) -> (impl Subscriber, Option<JoinHandle<()>>) {
    // Setup tracing
    let fmt_layer = tracing_subscriber::fmt::layer();

    let el_reg = tracing_subscriber::registry()
        .with(EnvFilter::new(&environ.rust_log))
        .with(fmt_layer);

    #[cfg(feature = "otlp")]
    let (el_reg, handle) = {
        let (el_reg, handle) = otlp(
            el_reg,
            environ.otlp_endpoint.clone(),
            environ.otlp_auth.clone(),
            environ.trace_attributes(),
            Duration::from_secs(30),
            cancel_token,
        );
        (el_reg, handle)
    };
    #[cfg(not(feature = "otlp"))]
    let handle = None;

    (el_reg, handle)
}

/// Handler for gracefully shutting down on Ctrl+c and SIGTERM
async fn shutdown_signal(cancel_token: CancellationToken, handle: Option<JoinHandle<()>>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for Ctrl+c event");
    };

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM event")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = cancel_token.cancelled() => {},
    }
    tracing::info!("Gracefully shutting down");
    cancel_token.cancel();
    if let Some(handle) = handle {
        handle.await.unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_tracing() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mock = server.mock("POST", "/v1/metrics").create_async().await;

        let rest_mock = server
            .mock("POST", mockito::Matcher::Any)
            // Expect no other requests
            .expect(0)
            .create_async()
            .await;

        let cancel_token = CancellationToken::new();
        let env = Env {
            otlp_endpoint: Some(url),
            otlp_auth: Some("unittest".to_string()),
            ..Env::default()
        };
        let (_tracing, handle) = setup_tracing(&env, cancel_token.clone());
        assert!(handle.is_some());

        // Cancel the background task and join its handle
        cancel_token.cancel();
        if let Some(handle) = handle {
            handle.await.unwrap();
        }
        mock.assert_async().await;
        rest_mock.assert_async().await;
    }

    #[tokio::test]
    // Test if no join handle is created when the OTLP env vars are not set
    // even though there is no use of async if this test passes, when it fails
    // it should fail on the assert, not on the lack of a tokio reactor
    // hence the #[tokio::test] here
    async fn setup_tracing_no_env() {
        let cancel_token = CancellationToken::new();
        let env = Env::default();
        let (_tracing, handle) = setup_tracing(&env, cancel_token.clone());
        assert!(handle.is_none());
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let cancel_token = CancellationToken::new();
        let upstream_cancel_token = cancel_token.clone();
        let shutdown_cancel_token = cancel_token.clone();

        // Create a handle to join in `shutdown_signal`
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = std::future::pending() => {},
                _ = upstream_cancel_token.cancelled() => {},
            }
        });
        // spawn `shutdown_signal`
        let handle = tokio::spawn(shutdown_signal(shutdown_cancel_token, Some(handle)));
        // Cancel both the upstream task and the shutdown_signal task
        cancel_token.cancel();
        handle.await.unwrap();
    }
}
