mod log;
mod trace;

use std::collections::HashMap;

use log::OtlpLogLayer;
use trace::OtlpTraceLayer;
use trace::SpanIdLayer;
use trace::SpanTimeLayer;
use tracing::Subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;

pub type OtlpLayer = (Option<OtlpLogLayer>, Option<OtlpTraceLayer>);

/// Wrap `subscriber` with OTLP tracing.
/// Note that this adds 4 types to every trace's extensions:
/// - [TraceId](opentelemetry::trace::TraceId) - ID shared by all nested spans
/// - [SpanId](opentelemetry::trace::SpanId) - ID of this span
/// - [SpanStart](trace::SpanStart) - Unix timestamp [ns] when the span was first entered
/// - [SpanEnd](trace::SpanEnd) - Unix timestamp [ns] when the span was last exited
pub fn otlp<S>(
    subscriber: S,
    otlp_endpoint: Option<String>,
    otlp_auth: Option<String>,
) -> (impl Subscriber, OtlpLayer)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (log_layer, trace_layer) = if let (Some(otlp_endpoint), Some(otlp_auth)) =
        (otlp_endpoint, otlp_auth)
    {
        let log_layer = crate::otlp::OtlpLogLayer::new(otlp_endpoint.clone(), otlp_auth.clone());
        let trace_layer = crate::otlp::OtlpTraceLayer::new(otlp_endpoint, otlp_auth);
        (Some(log_layer), Some(trace_layer))
    } else {
        (None, None)
    };

    (
        subscriber
            .with(SpanIdLayer::default())
            .with(SpanTimeLayer::default())
            .with(log_layer.clone())
            .with(trace_layer.clone()),
        (log_layer, trace_layer),
    )
}

pub trait Toilet {
    async fn flush(&self, _attributes: &HashMap<&str, Option<String>>) {}
}

impl<T> Toilet for Option<T>
where
    T: Toilet,
{
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        match self {
            Some(toilet) => toilet.flush(attributes).await,
            None => (),
        }
    }
}

impl Toilet for OtlpLayer {
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        self.0.flush(attributes).await;
        self.1.flush(attributes).await;
    }
}
