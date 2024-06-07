use std::collections::HashMap;
use std::env::set_var;

use async_trait::async_trait;
use bytes::Bytes;
use opentelemetry::logs::{LogRecord as _, Logger as _, LoggerProvider as _};
use opentelemetry_http::{HttpError, Request, Response};
use opentelemetry_sdk::logs::{Logger, LoggerProvider};
use tracing::Subscriber;
use tracing_core::Event;
use tracing_subscriber::{layer::Context, Layer};

// Compile time constants
// unfortunately we don't have access to the cloudflare env in the start handler
const OTEL_EXPORTER_OTLP_LOGS_ENDPOINT: &str = match option_env!("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
{
    Some(endpoint) => endpoint,
    None => "http://localhost:4317",
};
const OTEL_EXPORTER_OTLP_LOGS_AUTH: Option<&str> = option_env!("OTEL_EXPORTER_OTLP_LOGS_AUTH");

/// HTTP client for sending logs to an OTLP endpoint
#[derive(Debug, Default)]
struct HttpClient {
    client: reqwest::Client,
}

#[async_trait]
impl opentelemetry_http::HttpClient for HttpClient {
    async fn send(&self, request: Request<Vec<u8>>) -> Result<Response<Bytes>, HttpError> {
        let request = request.try_into()?;
        let mut response = self.client.execute(request).await?.error_for_status()?;
        let headers = std::mem::take(response.headers_mut());
        let mut http_response = Response::builder()
            .status(response.status())
            .body(response.bytes().await?)?;
        *http_response.headers_mut() = headers;

        Ok(http_response)
    }
}

// TODO: implement custom LogProcessor with flush() call to send logs
// TODO: implement Layer to convert events to logs

/// Construct a tracing layer for sending logs to an OTLP endpoint
fn otlp_layer() -> tracing_subscriber::Layer {
    // Exporter is the transport layer for the OTLP protocol
    // in this case we use protobuf over HTTP
    set_var(
        "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
        OTEL_EXPORTER_OTLP_LOGS_ENDPOINT,
    );
    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_http_client(HttpClient::default());
    let exporter = OTEL_EXPORTER_OTLP_LOGS_AUTH
        .map(|auth| {
            exporter.with_headers(HashMap::from([(
                "Authorization".to_string(),
                auth.to_string(),
            )]))
        })
        .unwrap_or(exporter);
    let exporter = exporter.build_log_exporter().unwrap();
    // Logger
    // TODO: create custom LogProcessor to batch logs
    let logger = LoggerProvider::builder()
        .with_simple_exporter(exporter)
        .build()
        .logger("pyoci");

    let mut record = logger.create_log_record();
    record.set_body("Hello, world!".into());
    logger.emit(record);
}

struct OtlpLogLayer {
    logger: Logger,
}

impl<S: Subscriber> Layer<S> for OtlpLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut record = self.logger.create_log_record();
        record.set_body(event.to_string().into());
        logger.emit(record);
    }
}
