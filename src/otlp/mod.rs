mod log;
mod metrics;
mod trace;

use metrics::OtlpMetricsLayer;
use std::collections::HashMap;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, MissedTickBehavior};

use log::OtlpLogLayer;
use tokio_util::sync::CancellationToken;
use trace::OtlpTraceLayer;
use trace::SpanIdLayer;
use trace::SpanTimeLayer;
use tracing::Subscriber;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;

/// Wrap `subscriber` with OTLP tracing.
/// Note that this adds 4 types to every trace's extensions:
/// - [`TraceId`](opentelemetry::trace::TraceId) - ID shared by all nested spans
/// - [`SpanId`](opentelemetry::trace::SpanId) - ID of this span
/// - [`SpanStart`](trace::SpanStart) - Unix timestamp [ns] when the span was first entered
/// - [`SpanEnd`](trace::SpanEnd) - Unix timestamp [ns] when the span was last exited
///
/// A background Task is spawned that will flush the records every `flush_interval`,
/// or when the `cancel_token` is canceled.
///
/// Returns the amended Subscriber and a `JoinHandle` for the background Task.
/// After canceling the `cancel_token`, await the `JoinHandle` to ensure everything gets flushed.
///
/// OTLP tracing won't be set up if `otlp_endpoint` or `otlp_auth` is None.
pub fn otlp<S>(
    subscriber: S,
    otlp_endpoint: Option<String>,
    otlp_auth: Option<String>,
    attributes: HashMap<&'static str, Option<String>>,
    flush_interval: Duration,
    cancel_token: CancellationToken,
) -> (Box<dyn Subscriber + Send + Sync>, Option<JoinHandle<()>>)
where
    S: Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
    let (Some(otlp_endpoint), Some(otlp_auth)) = (otlp_endpoint, otlp_auth) else {
        return (Box::new(subscriber), None);
    };
    let log_layer = crate::otlp::OtlpLogLayer::new(&otlp_endpoint, &otlp_auth);
    let trace_layer = crate::otlp::OtlpTraceLayer::new(&otlp_endpoint, &otlp_auth);
    let metrics_layer = crate::otlp::metrics::OtlpMetricsLayer::new(&otlp_endpoint, &otlp_auth);

    let subscriber = subscriber
        .with(SpanIdLayer::default())
        .with(SpanTimeLayer::default())
        .with(log_layer.clone())
        .with(trace_layer.clone())
        .with(metrics_layer.clone());
    let otlp_layer = (log_layer, trace_layer, metrics_layer);

    // A task that will flush every second
    let handle = tokio::spawn(async move {
        let mut interval = interval(flush_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                () = cancel_token.cancelled() => {
                    otlp_layer.flush(&attributes).await;
                    break;
                }
            }
            otlp_layer.flush(&attributes).await;
        }
    });
    (Box::new(subscriber), Some(handle))
}

pub trait Toilet {
    async fn flush(&self, _attributes: &HashMap<&str, Option<String>>);
}

type OtlpLayer = (OtlpLogLayer, OtlpTraceLayer, OtlpMetricsLayer);
impl Toilet for OtlpLayer {
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        self.0.flush(attributes).await;
        self.1.flush(attributes).await;
        self.2.flush(attributes).await;
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;
    use tracing::dispatcher;
    use tracing_subscriber::EnvFilter;

    use super::*;

    #[tokio::test]
    async fn otlp_layer_flush() {
        // init the mock server
        let mut server = mockito::Server::new_async().await;
        let url = server.url();
        let mocks = Vec::from([
            server
                .mock("POST", "/v1/logs")
                .match_header("Authorization", "unittest_auth")
                .match_header("Content-Type", "application/x-protobuf")
                .with_status(200)
                .create_async()
                .await,
            server
                .mock("POST", "/v1/traces")
                .match_header("Authorization", "unittest_auth")
                .match_header("Content-Type", "application/x-protobuf")
                .with_status(200)
                .create_async()
                .await,
        ]);

        let subscriber = tracing_subscriber::registry().with(EnvFilter::new("info"));
        let cancel_token = CancellationToken::new();

        let (subscriber, handle) = otlp(
            subscriber,
            Some(url),
            Some("unittest_auth".to_string()),
            HashMap::from([("service.name", Some("foo".to_string()))]),
            Duration::from_secs(1),
            cancel_token.clone(),
        );

        let dispatch = dispatcher::Dispatch::new(subscriber);
        dispatcher::with_default(&dispatch, || {
            let span = tracing::info_span!("unittest").entered();
            tracing::info!(target: "unittest", "unittest log 1");
            tracing::info!(target: "unittest", "unittest log 2");
            span.exit();
        });

        // Ensure flush gets called
        cancel_token.cancel();
        handle.unwrap().await.unwrap();

        for mock in mocks {
            mock.assert_async().await;
        }
    }
}
