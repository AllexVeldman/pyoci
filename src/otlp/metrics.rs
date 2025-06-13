use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use prost::Message;

use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::metrics::v1::{
    metric::Data, number_data_point::Value, AggregationTemporality, Metric, NumberDataPoint,
    ResourceMetrics, ScopeMetrics, Sum,
};
use opentelemetry_proto::tonic::resource::v1::Resource;
use tracing::span::{Attributes, Id};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use crate::otlp::Toilet;
use crate::time::time_unix_ns;
use crate::USER_AGENT;

/// Set of metrics to track
#[derive(Debug)]
struct Metrics {
    uptime: UptimeMetric,
    requests: RequestsMetric,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            uptime: UptimeMetric::new(),
            requests: RequestsMetric::new(),
        }
    }
}

impl Metrics {
    fn as_metrics(&self, attributes: &[KeyValue]) -> Vec<Metric> {
        vec![
            self.uptime.as_metric(attributes),
            self.requests.as_metric(attributes),
        ]
    }
}

#[derive(Debug)]
struct UptimeMetric {
    /// Moment this metric started measuring
    start_ns: u64,
}

impl UptimeMetric {
    fn new() -> Self {
        Self {
            start_ns: time_unix_ns(),
        }
    }

    fn as_metric(&self, attributes: &[KeyValue]) -> Metric {
        let now = time_unix_ns();
        let diff = (now - self.start_ns) as f64 / 1_000_000_000.0;
        Metric {
            name: "pyoci_uptime".to_string(),
            description: "Time in seconds this instance has been running".to_string(),
            unit: "seconds".to_string(),
            data: Some(Data::Sum(Sum {
                data_points: vec![NumberDataPoint {
                    attributes: attributes.to_vec(),
                    start_time_unix_nano: now,
                    time_unix_nano: now,
                    value: Some(Value::AsDouble(diff)),
                    ..NumberDataPoint::default()
                }],
                aggregation_temporality: AggregationTemporality::Cumulative.into(),
                is_monotonic: true,
            })),
            metadata: vec![],
        }
    }
}

#[derive(Debug)]
struct RequestsMetric {
    count: RwLock<u32>,
}

impl RequestsMetric {
    fn new() -> RequestsMetric {
        RequestsMetric {
            count: RwLock::new(0),
        }
    }

    fn increment(&self) {
        *self.count.write().unwrap() += 1;
    }

    fn as_metric(&self, attributes: &[KeyValue]) -> Metric {
        let now = time_unix_ns();
        Metric {
            name: "pyoci_requests".to_string(),
            description: "Total number of requests handled by this instance".to_string(),
            unit: "requests".to_string(),
            data: Some(Data::Sum(Sum {
                data_points: vec![NumberDataPoint {
                    attributes: attributes.to_vec(),
                    start_time_unix_nano: now,
                    time_unix_nano: now,
                    value: Some(Value::AsInt(*self.count.read().unwrap() as i64)),
                    ..NumberDataPoint::default()
                }],
                aggregation_temporality: AggregationTemporality::Cumulative.into(),
                is_monotonic: true,
            })),
            metadata: vec![],
        }
    }
}

/// Convert metrics into a ExportMetricsServiceRequest
/// <https://opentelemetry.io/docs/specs/otlp/#otlpgrpc>
fn build_metrics_export_body(
    metrics: &Metrics,
    attributes: &HashMap<&str, Option<String>>,
) -> ExportMetricsServiceRequest {
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
    let scope_metrics = ScopeMetrics {
        scope: None,
        metrics: metrics.as_metrics(&attrs),
        schema_url: "".to_string(),
    };
    let resource_metrics = ResourceMetrics {
        resource: Some(Resource {
            attributes: attrs,
            ..Resource::default()
        }),
        scope_metrics: vec![scope_metrics],
        schema_url: "".to_string(),
    };
    ExportMetricsServiceRequest {
        resource_metrics: vec![resource_metrics],
    }
}

/// Tracing Layer for pushing metrics to an OTLP consumer.
#[derive(Debug, Clone)]
pub struct OtlpMetricsLayer {
    otlp_endpoint: String,
    otlp_auth: String,
    /// Buffer of Metrics
    metrics: Arc<Metrics>,
}

// Public methods
impl OtlpMetricsLayer {
    pub fn new(otlp_endpoint: &str, otlp_auth: &str) -> Self {
        Self {
            otlp_endpoint: otlp_endpoint.to_string(),
            otlp_auth: otlp_auth.to_string(),
            metrics: Arc::new(Metrics::default()),
        }
    }
}

impl<S> Layer<S> for OtlpMetricsLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            tracing::info!("Span {id:?} does not exist");
            return;
        };

        // If this is the root span, we are in a new request
        if span.parent().is_none() {
            self.metrics.requests.increment();
        }
    }
}

impl Toilet for OtlpMetricsLayer {
    /// Push all recorded log messages to the OTLP collector
    /// This should be called at the end of every request, after the span is closed
    async fn flush(&self, attributes: &HashMap<&str, Option<String>>) {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        let body = build_metrics_export_body(&self.metrics, attributes).encode_to_vec();
        let mut url = url::Url::parse(&self.otlp_endpoint).unwrap();
        url.path_segments_mut().unwrap().extend(&["v1", "metrics"]);
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
                    tracing::info!("Failed to send metrics to OTLP: {:?}", response);
                    tracing::info!("Response body: {:?}", response.text().await.unwrap());
                } else {
                    tracing::info!("Metrics sent to OTLP: {:?}", response);
                };
            }
            Err(err) => {
                tracing::info!("Error sending metrics to OTLP: {:?}", err);
            }
        };
    }
}
