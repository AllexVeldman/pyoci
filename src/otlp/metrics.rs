use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use prost::Message;
use time::OffsetDateTime;

use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::metrics::v1::{
    metric::Data, number_data_point::Value, AggregationTemporality, Metric, NumberDataPoint,
    ResourceMetrics, ScopeMetrics, Sum,
};
use opentelemetry_proto::tonic::resource::v1::Resource;

use crate::otlp::Toilet;
use crate::USER_AGENT;

/// Set of metrics to track
#[derive(Debug)]
struct Metrics {
    uptime: UptimeMetric,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            uptime: UptimeMetric::new(),
        }
    }
}

impl Metrics {
    fn as_metrics(&self, attributes: &[KeyValue]) -> Vec<Metric> {
        vec![self.uptime.as_metric(attributes)]
    }
}

#[derive(Debug)]
struct UptimeMetric {
    /// Moment this metric started measuring
    start_ns: f64,
}

impl UptimeMetric {
    fn new() -> Self {
        Self {
            start_ns: OffsetDateTime::now_utc().unix_timestamp_nanos() as f64,
        }
    }

    fn as_metric(&self, attributes: &[KeyValue]) -> Metric {
        let now = OffsetDateTime::now_utc().unix_timestamp_nanos();
        let now_u64: u64 = now.try_into().expect("timestamp does not fit in u64");
        let diff = (now as f64 - self.start_ns) / 1_000_000_000.0;
        Metric {
            name: "pyoci_uptime".to_string(),
            description: "Time in seconds this instance has been running".to_string(),
            unit: "seconds".to_string(),
            data: Some(Data::Sum(Sum {
                data_points: vec![NumberDataPoint {
                    attributes: attributes.to_vec(),
                    start_time_unix_nano: now_u64,
                    time_unix_nano: now_u64,
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
            dropped_attributes_count: 0,
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
