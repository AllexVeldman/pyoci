use std::collections::HashMap;
use std::fmt::{self, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use prost::Message;
use tracing::Subscriber;
use tracing_core::Event;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use tracing::field::{Field, Visit};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::resource::v1::Resource;

use crate::otlp::trace::{SpanId, TraceId};
use crate::otlp::Toilet;
use crate::time::time_unix_ns;
use crate::USER_AGENT;

/// Convert a batch of log records into a `ExportLogsServiceRequest`
/// <https://opentelemetry.io/docs/specs/otlp/#otlpgrpc>
fn build_logs_export_body(
    logs: Vec<LogRecord>,
    attributes: &HashMap<&str, Option<String>>,
) -> ExportLogsServiceRequest {
    let scope_logs = ScopeLogs {
        scope: None,
        log_records: logs,
        schema_url: String::new(),
    };

    let mut attrs = vec![];
    for (key, value) in attributes {
        let Some(value) = value else {
            continue;
        };
        attrs.push(KeyValue {
            key: (*key).into(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(value.into())),
            }),
        });
    }
    let resource_logs = ResourceLogs {
        resource: Some(Resource {
            attributes: attrs,
            ..Resource::default()
        }),
        scope_logs: vec![scope_logs],
        schema_url: String::new(),
    };
    ExportLogsServiceRequest {
        resource_logs: vec![resource_logs],
    }
}

/// Relies on [`TraceId`] and [`SpanId`] to be available in the Event's Span, see [`crate::otlp::trace::SpanIdLayer`]
/// Tracing Layer for pushing logs to an OTLP consumer.
#[derive(Debug, Clone)]
pub struct OtlpLogLayer {
    otlp_endpoint: String,
    otlp_auth: String,
    /// Buffer of `LogRecords`, each (log) event during a request will be added to this buffer
    records: Arc<RwLock<Vec<LogRecord>>>,
}

// Public methods
impl OtlpLogLayer {
    pub fn new(otlp_endpoint: &str, otlp_auth: &str) -> Self {
        Self {
            otlp_endpoint: otlp_endpoint.to_string(),
            otlp_auth: otlp_auth.to_string(),
            records: Arc::new(RwLock::new(vec![])),
        }
    }
}

impl Toilet for OtlpLogLayer {
    /// Push all recorded log messages to the OTLP collector
    /// This should be called at the end of every request, after the span is closed
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        let records: Vec<LogRecord> = self.records.write().unwrap().drain(..).collect();
        if records.is_empty() {
            tracing::debug!("No logs to send");
            return;
        }
        tracing::info!("Sending {} log records to OTLP", records.len());
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        let body = build_logs_export_body(records, attributes).encode_to_vec();
        let mut url = url::Url::parse(&self.otlp_endpoint).unwrap();
        url.path_segments_mut().unwrap().extend(&["v1", "logs"]);
        // send to OTLP Collector
        match client
            .post(url)
            .header("Content-Type", "application/x-protobuf")
            .header("Authorization", &self.otlp_auth)
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    tracing::info!("Logs sent to OTLP: {:?}", response);
                } else {
                    tracing::info!("Failed to send logs to OTLP: {:?}", response);
                    tracing::info!("Response body: {:?}", response.text().await.unwrap());
                }
            }
            Err(err) => {
                tracing::info!("Error sending logs to OTLP: {:?}", err);
            }
        }
    }
}

impl<S> Layer<S> for OtlpLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let time_ns = time_unix_ns();

        let metadata = event.metadata();
        // Drop any logs generated as part of the otlp module
        if metadata.target().contains("otlp") {
            return;
        }
        // Get the log level and message
        let level = metadata.level();
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let Some(span) = ctx.event_span(event) else {
            tracing::debug!("Currently not in a span");
            return;
        };

        let extensions = span.extensions();
        let Some(trace_id) = extensions.get::<TraceId>() else {
            tracing::error!("Could not find Trace ID for Span {:?}", span.id());
            return;
        };

        let Some(span_id) = extensions.get::<SpanId>() else {
            tracing::error!("Could not find Span ID for Span {:?}", span.id());
            return;
        };

        let log_record = LogRecord {
            time_unix_nano: time_ns,
            observed_time_unix_nano: time_ns,
            severity_text: level.to_string().to_uppercase(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue(
                    visitor.string.trim().to_string(),
                )),
            }),
            attributes: vec![],
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            ..LogRecord::default()
        };

        self.records.write().unwrap().push(log_record);
    }
}

#[derive(Default)]
pub struct LogVisitor {
    // The log message
    string: String,
}

impl Visit for LogVisitor {
    fn record_debug(&mut self, _field: &Field, value: &dyn fmt::Debug) {
        write!(self.string, "{value:?} ").unwrap();
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        write!(self.string, "{}=\"{}\" ", field.name(), value).unwrap();
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        write!(self.string, "{}={} ", field.name(), value).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::otlp::SpanIdLayer;

    use super::*;
    use tracing::dispatcher;
    use tracing_core::LevelFilter;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn otlp_log_layer() {
        // init the mock server
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mock = server
            .mock("POST", "/v1/logs")
            .match_header("Authorization", "unittest_auth")
            .match_header("Content-Type", "application/x-protobuf")
            .with_status(200)
            .create_async()
            .await;

        // init tracing with the otlp layer
        let otlp_layer = OtlpLogLayer::new(&url, "unittest_auth");
        let otlp_clone = otlp_layer.clone();
        let subscriber = tracing_subscriber::registry()
            .with(SpanIdLayer::default())
            .with(otlp_layer.with_filter(LevelFilter::INFO));
        // Set the subscriber as the default within the scope of the logs
        // This allows us to run tests in parallel, all setting their own subscriber
        let dispatch = dispatcher::Dispatch::new(subscriber);
        dispatcher::with_default(&dispatch, || {
            let span = tracing::info_span!("unittest").entered();
            tracing::info!(target: "unittest", status=200_u16, path="/", "unittest log 1");
            tracing::info!(target: "unittest", "unittest log 2");
            tracing::info!(target: "unittest", "unittest log 3");
            tracing::info!(target: "unittest", "unittest log 4");
            span.exit();
        });

        // I would like to validate the body here but since mockito requires an exact match for
        // Vec[u8], there are timestamps in the body, and I have no way of stopping time during
        // tests, I don't (yet) know how to do that.
        assert_eq!(otlp_clone.records.read().unwrap().len(), 4);
        assert_eq!(
            otlp_clone.records.read().unwrap()[0].body.as_ref().unwrap(),
            &AnyValue {
                value: Some(any_value::Value::StringValue(
                    "unittest log 1 status=200 path=\"/\"".into()
                )),
            }
        );
        otlp_clone
            .flush(&HashMap::from([("unittest", Some("test1".into()))]))
            .await;

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn otlp_log_layer_no_records() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mock = server
            .mock("POST", mockito::Matcher::Any)
            // Expect no requests
            .expect(0)
            .create_async()
            .await;

        // init tracing with the otlp layer
        let otlp_layer = OtlpLogLayer::new(&url, "");
        let otlp_clone = otlp_layer.clone();
        let subscriber = tracing_subscriber::registry()
            .with(SpanIdLayer::default())
            .with(otlp_layer.with_filter(LevelFilter::INFO));
        let dispatch = dispatcher::Dispatch::new(subscriber);
        dispatcher::with_default(&dispatch, || {
            // create a span and exit it without any logs happening
            let span = tracing::info_span!("unittest").entered();
            tracing::info!("Warning not for OTLP!");
            span.exit();
        });

        assert_eq!(otlp_clone.records.read().unwrap().len(), 0);
        otlp_clone.flush(&HashMap::from([("unittest", None)])).await;

        mock.assert_async().await;
    }
}
