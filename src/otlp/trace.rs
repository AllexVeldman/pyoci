use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status};
use prost::Message;
use time::OffsetDateTime;
use tracing::span::Attributes;
use tracing::Id;
use tracing::Subscriber;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use opentelemetry::trace::{SpanId, TraceId};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_sdk::trace::{IdGenerator, RandomIdGenerator};

use super::log::log;
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
#[derive(Debug, Clone)]
pub struct OtlpTraceLayer {
    otlp_endpoint: String,
    otlp_auth: String,
    /// Buffer of Spans
    spans: Arc<RwLock<Vec<Span>>>,
}

// Public methods
impl OtlpTraceLayer {
    pub fn new(otlp_endpoint: String, otlp_auth: String) -> Self {
        Self {
            otlp_endpoint,
            otlp_auth,
            spans: Arc::new(RwLock::new(vec![])),
        }
    }
}

// Private methods
impl OtlpTraceLayer {
    /// Push all recorded log messages to the OTLP collector
    /// This should be called at the end of every request, after the span is closed
    pub async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        let spans: Vec<Span> = self.spans.write().unwrap().drain(..).collect();
        if spans.is_empty() {
            log!("No spans to send");
            return;
        }
        log!("Sending {} spans to OTLP", spans.len());
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
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
                    log!("Failed to send traces to OTLP: {:?}", response);
                    log!("Response body: {:?}", response.text().await.unwrap());
                } else {
                    log!("Traces sent to OTLP: {:?}", response);
                };
            }
            Err(err) => {
                log!("Error sending traces to OTLP: {:?}", err);
            }
        };
    }
}

impl<S> Layer<S> for OtlpTraceLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Add TraceId and SpanId to the span extensions
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            log!("Span {id:?} does not exist");
            return;
        };
        let mut extensions = span.extensions_mut();
        let rng = RandomIdGenerator::default();
        extensions.insert(rng.new_span_id());

        match span.parent() {
            None => extensions.insert(rng.new_trace_id()),
            Some(parent) => extensions.insert(
                *parent
                    .extensions()
                    .get::<TraceId>()
                    .expect("TraceId not set, this is a bug"),
            ),
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            log!("Span {id:?} does not exist");
            return;
        };
        let extensions = span.extensions();
        let Some(start_time) = extensions.get::<SpanStart>() else {
            log!("SpanStart not defined for Span {id:?}");
            return;
        };
        let Some(end_time) = extensions.get::<SpanEnd>() else {
            log!("SpanEnd not defined for Span {id:?}");
            return;
        };

        let extensions = span.extensions();
        let Some(trace_id) = extensions.get::<TraceId>() else {
            log!("Could not find Trace ID for Span {id:?}");
            return;
        };

        let Some(span_id) = extensions.get::<SpanId>() else {
            log!("Could not find Span ID for Span {id:?}");
            return;
        };

        let parent_span_id = span
            .parent()
            .map(|p_span| p_span.extensions().get::<SpanId>().map(|id| id.to_bytes()))
            .unwrap_or_default()
            .unwrap_or_default()
            .into();

        let span = Span {
            trace_id: trace_id.to_bytes().into(),
            span_id: span_id.to_bytes().into(),
            parent_span_id,
            trace_state: "".to_string(),
            flags: 0,
            name: span.name().to_string(),
            // TODO: fetch this from the span fields
            kind: SpanKind::Internal.into(),
            start_time_unix_nano: start_time.into(),
            end_time_unix_nano: end_time.into(),
            //TODO: Add attrs from span.fields
            attributes: vec![],
            dropped_attributes_count: 0,
            // Are these the logs?
            events: vec![],
            dropped_events_count: 0,
            links: vec![],
            dropped_links_count: 0,
            status: Some(Status {
                message: "".to_string(),
                code: StatusCode::Ok.into(),
            }),
        };

        self.spans.write().unwrap().push(span);
    }
}

fn time_unix_ns() -> Option<u64> {
    match OffsetDateTime::now_utc().unix_timestamp_nanos().try_into() {
        Ok(value) => Some(value),
        Err(_) => {
            log!("SystemTime out of range for conversion to u64!");
            None
        }
    }
}

#[derive(Debug)]
struct SpanStart(u64);

#[derive(Debug)]
struct SpanEnd(u64);

impl From<&SpanStart> for u64 {
    fn from(value: &SpanStart) -> u64 {
        value.0
    }
}

impl From<&SpanEnd> for u64 {
    fn from(value: &SpanEnd) -> u64 {
        value.0
    }
}

/// Inject span timings into the span extensions, required by OtlpTraceLayer
pub struct SpanTimeLayer {}
impl SpanTimeLayer {
    pub fn new() -> Self {
        Self {}
    }
}

impl<S> Layer<S> for SpanTimeLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// Insert the SpanStart when we enter this span
    /// note that a span is entered and exited when crossing await bounds
    /// so we should only set the start value once.
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if span.extensions().get::<SpanStart>().is_some() {
            return;
        };
        let Some(current_time) = time_unix_ns().map(SpanStart) else {
            return;
        };
        span.extensions_mut().replace::<SpanStart>(current_time);
    }
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let Some(current_time) = time_unix_ns().map(SpanEnd) else {
            return;
        };
        span.extensions_mut().replace::<SpanEnd>(current_time);
    }
}
