use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use prost::Message;
use time::OffsetDateTime;
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::Id;
use tracing::Subscriber;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry::Value;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_sdk::trace::{IdGenerator, RandomIdGenerator};

use crate::otlp::Toilet;
use crate::USER_AGENT;

/// <https://opentelemetry.io/docs/specs/otlp/#otlpgrpc>
fn build_trace_export_body(
    spans: Vec<Span>,
    attributes: &HashMap<&str, Option<String>>,
) -> ExportTraceServiceRequest {
    let scope_spans = ScopeSpans {
        scope: None,
        spans,
        schema_url: "".to_string(),
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
    let resource_spans = ResourceSpans {
        resource: Some(Resource {
            attributes: attrs,
            dropped_attributes_count: 0,
        }),
        scope_spans: vec![scope_spans],
        schema_url: "".to_string(),
    };
    ExportTraceServiceRequest {
        resource_spans: vec![resource_spans],
    }
}

/// Tracing Layer for pushing logs to an OTLP consumer.
/// Requires a [TraceId] and [SpanId] to be present in Trace Extensions, see [SpanIdLayer].
/// Requires [SpanStart] and [SpanEnd] to be present in the Trace Extensions, see [SpanTimeLayer].
#[derive(Debug, Clone)]
pub struct OtlpTraceLayer {
    otlp_endpoint: String,
    otlp_auth: String,
    /// Buffer of Spans
    spans: Arc<RwLock<Vec<Span>>>,
}

// Public methods
impl OtlpTraceLayer {
    pub fn new(otlp_endpoint: &str, otlp_auth: &str) -> Self {
        Self {
            otlp_endpoint: otlp_endpoint.to_string(),
            otlp_auth: otlp_auth.to_string(),
            spans: Arc::new(RwLock::new(vec![])),
        }
    }
}

// Private methods
impl Toilet for OtlpTraceLayer {
    /// Push all recorded log messages to the OTLP collector
    /// This should be called at the end of every request, after the span is closed
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        let spans: Vec<Span> = self.spans.write().unwrap().drain(..).collect();
        if spans.is_empty() {
            tracing::debug!("No spans to send");
            return;
        }
        tracing::info!("Sending {} spans to OTLP", spans.len());
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        let body = build_trace_export_body(spans, attributes).encode_to_vec();
        let mut url = url::Url::parse(&self.otlp_endpoint).unwrap();
        url.path_segments_mut().unwrap().extend(&["v1", "traces"]);
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
                if !response.status().is_success() {
                    tracing::info!("Failed to send traces to OTLP: {:?}", response);
                    tracing::info!("Response body: {:?}", response.text().await.unwrap());
                } else {
                    tracing::info!("Traces sent to OTLP: {:?}", response);
                };
            }
            Err(err) => {
                tracing::info!("Error sending traces to OTLP: {:?}", err);
            }
        };
    }
}

impl<S> Layer<S> for OtlpTraceLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Insert a new Span in the spans Extensions
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            tracing::info!("Span {id:?} does not exist");
            return;
        };
        let otel_span = {
            let extensions = span.extensions();
            let Some(trace_id) = extensions.get::<TraceId>() else {
                tracing::info!("Could not find Trace ID for Span {id:?}");
                return;
            };

            let Some(span_id) = extensions.get::<SpanId>() else {
                tracing::info!("Could not find Span ID for Span {id:?}");
                return;
            };

            let parent_span_id = span
                .parent()
                .map(|p_span| p_span.extensions().get::<SpanId>().map(|id| id.to_bytes()))
                .unwrap_or_default()
                .unwrap_or_default()
                .into();
            let mut visitor = OtelVisitor::default();
            attrs.record(&mut visitor);

            Span {
                trace_id: trace_id.to_bytes().into(),
                span_id: span_id.to_bytes().into(),
                parent_span_id,
                name: span.name().to_string(),
                kind: visitor.kind.into(),
                attributes: visitor.attributes,
                ..Span::default()
            }
        };
        let mut extensions = span.extensions_mut();
        extensions.insert(otel_span);
    }

    /// Pull the Span from the span extensions and push it onto the spans buffer
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            tracing::info!("Span {id:?} does not exist");
            return;
        };
        let (start_time, end_time) = {
            let extensions = span.extensions();
            let Some(start_time) = extensions.get::<SpanEnter>() else {
                tracing::info!("SpanStart not defined for Span {id:?}");
                return;
            };
            let Some(end_time) = extensions.get::<SpanExit>() else {
                tracing::info!("SpanEnd not defined for Span {id:?}");
                return;
            };
            (start_time.into(), end_time.into())
        };
        let mut extensions = span.extensions_mut();
        let Some(mut span) = extensions.remove::<Span>() else {
            tracing::info!("Span not defined for Span {id:?}");
            return;
        };
        span.start_time_unix_nano = start_time;
        span.end_time_unix_nano = end_time;

        self.spans.write().unwrap().push(span);
    }
}

/// Collect Otel attributes from trace Attribute's
#[derive(Debug)]
struct OtelVisitor {
    kind: SpanKind,
    attributes: Vec<KeyValue>,
}

impl Default for OtelVisitor {
    fn default() -> Self {
        Self {
            kind: SpanKind::Internal,
            attributes: vec![],
        }
    }
}

impl Visit for OtelVisitor {
    fn record_debug(&mut self, _field: &Field, _value: &dyn core::fmt::Debug) {
        // do nothing
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "otel.span_kind" {
            if let Some(kind) =
                SpanKind::from_str_name(&format!("SPAN_KIND_{}", value.to_uppercase()))
            {
                self.kind = kind
            }
        } else if let Some(key) = name.strip_prefix("otel.") {
            self.attributes.push(KeyValue {
                key: key.into(),
                value: Some(AnyValue::from(Value::from(value.to_string()))),
            })
        }
    }
}

fn time_unix_ns() -> Option<u64> {
    match OffsetDateTime::now_utc().unix_timestamp_nanos().try_into() {
        Ok(value) => Some(value),
        Err(_) => {
            tracing::info!("SystemTime out of range for conversion to u64!");
            None
        }
    }
}

/// Unix timestamp (ns) when this Span was first entered.
#[derive(Debug)]
pub struct SpanEnter(u64);

/// Unix timestamp (ns) when this Span was last exited.
#[derive(Debug)]
pub struct SpanExit(u64);

impl From<&SpanEnter> for u64 {
    fn from(value: &SpanEnter) -> u64 {
        value.0
    }
}

impl From<&SpanExit> for u64 {
    fn from(value: &SpanExit) -> u64 {
        value.0
    }
}

/// Inject span timings into the span extensions, required by OtlpTraceLayer
#[derive(Debug, Default)]
pub struct SpanTimeLayer {}

impl<S> Layer<S> for SpanTimeLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Insert the SpanStart when we enter this span
    /// note that a span is entered and exited when crossing await bounds
    /// so we should only set the start value once.
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if span.extensions().get::<SpanEnter>().is_some() {
            return;
        };
        let Some(current_time) = time_unix_ns().map(SpanEnter) else {
            return;
        };
        span.extensions_mut().replace::<SpanEnter>(current_time);
    }
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let Some(current_time) = time_unix_ns().map(SpanExit) else {
            return;
        };
        span.extensions_mut().replace::<SpanExit>(current_time);
    }
}

#[derive(Debug, Default)]
pub struct SpanIdLayer {
    rng: RandomIdGenerator,
}

impl<S> Layer<S> for SpanIdLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            tracing::info!("Span {id:?} does not exist");
            return;
        };
        let mut extensions = span.extensions_mut();
        extensions.insert(self.rng.new_span_id());

        match span.parent() {
            None => extensions.insert(self.rng.new_trace_id()),
            Some(parent) => extensions.insert(
                *parent
                    .extensions()
                    .get::<TraceId>()
                    .expect("TraceId not set, this is a bug"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use tracing::dispatcher;
    use tracing_core::LevelFilter;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn otlp_trace_layer() {
        // init the mock server
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mock = server
            .mock("POST", "/v1/traces")
            .match_header("Authorization", "unittest_auth")
            .match_header("Content-Type", "application/x-protobuf")
            .with_status(200)
            .create_async()
            .await;

        // init tracing with the otlp layer
        let otlp_layer = OtlpTraceLayer::new(&url, "unittest_auth");
        let otlp_clone = otlp_layer.clone();
        let subscriber = tracing_subscriber::registry()
            .with(SpanIdLayer::default())
            .with(SpanTimeLayer::default())
            .with(otlp_layer.with_filter(LevelFilter::INFO));
        // Set the subscriber as the default within the scope of the logs
        // This allows us to run tests in parallel, all setting their own subscriber
        let dispatch = dispatcher::Dispatch::new(subscriber);
        dispatcher::with_default(&dispatch, || {
            let span = tracing::info_span!("unittest").entered();
            let subspan = tracing::info_span!("subspan1").entered();
            tracing::info_span!("subspan2").entered().exit();
            subspan.exit();
            span.exit();
        });
        {
            let spans = otlp_clone.spans.read().unwrap();
            assert_eq!(spans.len(), 3);
            // We store spans on_close, to they index in reverse order here
            assert_eq!(spans[2].name, "unittest");
            let trace_id = &spans[2].trace_id;
            assert_eq!(spans[1].name, "subspan1");
            assert_eq!(&spans[1].trace_id, trace_id);
            assert_eq!(&spans[1].parent_span_id, &spans[2].span_id);
            assert_eq!(spans[0].name, "subspan2");
            assert_eq!(&spans[0].trace_id, trace_id);
            assert_eq!(&spans[0].parent_span_id, &spans[1].span_id);
        }
        otlp_clone
            .flush(&HashMap::from([("unittest", Some("test1".into()))]))
            .await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn otlp_trace_layer_no_records() {
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mock = server
            .mock("POST", mockito::Matcher::Any)
            // Expect no requests
            .expect(0)
            .create_async()
            .await;

        // init tracing with the otlp layer
        let otlp_layer = OtlpTraceLayer::new(&url, "");
        let otlp_clone = otlp_layer.clone();
        let subscriber = tracing_subscriber::registry()
            .with(SpanIdLayer::default())
            .with(SpanTimeLayer::default())
            .with(otlp_layer.with_filter(LevelFilter::INFO));
        let dispatch = dispatcher::Dispatch::new(subscriber);
        dispatcher::with_default(&dispatch, || {
            // Nothing happens during the dispatch
        });

        assert_eq!(otlp_clone.spans.read().unwrap().len(), 0);
        otlp_clone.flush(&HashMap::from([("unittest", None)])).await;

        mock.assert_async().await;
    }
}
