use std::collections::HashMap;
use std::fmt::{self, Write};
use std::sync::RwLock;

use anyhow::Result;
use prost::Message;
use time::OffsetDateTime;
use tracing::Subscriber;
use tracing_core::Event;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};
use worker::console_log;

use tracing::field::{Field, Visit};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::resource::v1::Resource;

use crate::USER_AGENT;

/// Convert a batch of log records into a ExportLogsServiceRequest
/// <https://opentelemetry.io/docs/specs/otlp/#otlpgrpc>
fn build_logs_export_body(
    logs: Vec<LogRecord>,
    attributes: &HashMap<String, Option<String>>,
) -> ExportLogsServiceRequest {
    let scope_logs = ScopeLogs {
        scope: None,
        log_records: logs,
        schema_url: "".to_string(),
    };

    let mut attrs = vec![];
    for (key, value) in attributes {
        let Some(value) = value else {
            continue;
        };
        attrs.push(KeyValue {
            key: key.into(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(value.into())),
            }),
        });
    }
    let resource_logs = ResourceLogs {
        resource: Some(Resource {
            attributes: attrs,
            dropped_attributes_count: 0,
        }),
        scope_logs: vec![scope_logs],
        schema_url: "".to_string(),
    };
    ExportLogsServiceRequest {
        resource_logs: vec![resource_logs],
    }
}

pub struct OtlpLogLayer {
    records: RwLock<Vec<LogRecord>>,
    sender: async_channel::Sender<Vec<LogRecord>>,
}

// Public methods
impl OtlpLogLayer {
    pub fn new() -> (Self, async_channel::Receiver<Vec<LogRecord>>) {
        let (sender, receiver) = async_channel::bounded(10);
        (
            Self {
                records: RwLock::new(vec![]),
                sender,
            },
            receiver,
        )
    }
}

/// Flush all messages from the queue
/// In normal operation this will be called after every request and should only every consume 1
/// message
pub async fn flush(
    receiver: &async_channel::Receiver<Vec<LogRecord>>,
    otlp_endpoint: String,
    otlp_auth: String,
    attributes: &HashMap<String, Option<String>>,
) {
    console_log!("Flushing logs to OTLP");
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap();
    loop {
        // Wait for messages from the OtlpLogLayer
        // These are send at the end of every request
        let log_records = match receiver.recv().await {
            Ok(request) => request,
            // Channel is empty and closed, so we're done here
            Err(_) => {
                console_log!("Channel is empty and closed");
                break;
            }
        };
        let body = build_logs_export_body(log_records, attributes).encode_to_vec();
        let mut url = url::Url::parse(&otlp_endpoint).unwrap();
        url.path_segments_mut().unwrap().extend(&["v1", "logs"]);
        // send to OTLP Collector
        match client
            .post(url)
            .header("Content-Type", "application/x-protobuf")
            .header("Authorization", &otlp_auth)
            .body(body)
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    console_log!("Failed to send logs to OTLP: {:?}", response);
                    console_log!("Response body: {:?}", response.text().await.unwrap());
                } else {
                    console_log!("Logs sent to OTLP: {:?}", response);
                };
            }
            Err(err) => {
                console_log!("Error sending logs to OTLP: {:?}", err);
            }
        };
        // If the channel is empty, we're done
        // New messages will be handled by the next request
        if receiver.is_empty() {
            break;
        }
    }
}

// Private methods
impl OtlpLogLayer {
    /// Push all recorded log messages onto the channel
    /// This is called at the end of every request
    fn flush(&self) -> Result<()> {
        let records: Vec<LogRecord> = self.records.write().unwrap().drain(..).collect();
        console_log!("Sending {} log records to OTLP", records.len());
        self.sender.try_send(records)?;
        Ok(())
    }
}

impl<S> Layer<S> for OtlpLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(time_ns) = time_unix_ns() else {
            return;
        };

        // Get the log level and message
        let level = event.metadata().level();
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        // Extract the span and trace IDs
        // we'll consider the current span ID as the trace ID when there is no parent span
        let (trace_id, span_id) = match ctx.event_span(event) {
            Some(mut span) => {
                let mut span_id = span.id().into_u64().to_be_bytes().to_vec();
                span_id.resize(8, 0);
                let mut trace_id = loop {
                    match span.parent() {
                        Some(parent) => {
                            span = parent;
                        }
                        None => {
                            break span.id().into_u64().to_be_bytes().to_vec();
                        }
                    }
                };
                trace_id.resize(16, 0);
                (Some(trace_id), Some(span_id))
            }
            None => {
                let mut span_id = ctx
                    .current_span()
                    .id()
                    .map(|id| id.into_u64().to_be_bytes().to_vec());
                let mut trace_id = span_id.clone();
                if let Some(id) = span_id.as_mut() {
                    id.resize(8, 0)
                }
                if let Some(id) = trace_id.as_mut() {
                    id.resize(16, 0)
                }
                (span_id, trace_id)
            }
        };

        let log_record = LogRecord {
            time_unix_nano: time_ns,
            observed_time_unix_nano: time_ns,
            severity_number: severity_number(level),
            severity_text: level.to_string().to_uppercase(),
            body: Some(AnyValue {
                value: Some(any_value::Value::StringValue(visitor.string)),
            }),
            attributes: vec![],
            dropped_attributes_count: 0,
            trace_id: trace_id.unwrap_or_default(),
            span_id: span_id.unwrap_or_default(),
            flags: 0,
        };

        self.records.write().unwrap().push(log_record);
    }

    fn on_close(&self, id: tracing_core::span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("span not found");
        if !span.parent().is_none() {
            // This is a sub-span, we'll flush all messages when the root span is closed
            return;
        }
        if let Err(err) = self.flush() {
            console_log!("Failed to flush log records: {:?}", err);
        }
    }
}

fn time_unix_ns() -> Option<u64> {
    match OffsetDateTime::now_utc().unix_timestamp_nanos().try_into() {
        Ok(value) => Some(value),
        Err(_) => {
            console_log!("SystemTime out of range for conversion to u64!");
            None
        }
    }
}

fn severity_number(level: &tracing::Level) -> i32 {
    match *level {
        tracing::Level::ERROR => 17,
        tracing::Level::WARN => 13,
        tracing::Level::INFO => 9,
        tracing::Level::DEBUG => 5,
        tracing::Level::TRACE => 1,
    }
}

#[derive(Default)]
pub struct LogVisitor {
    string: String,
}

impl Visit for LogVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        write!(self.string, "{}={:?} ", field.name(), value).unwrap();
    }
}
