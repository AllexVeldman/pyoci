use std::fmt::{self, Write};
use std::sync::RwLock;

use anyhow::Result;
use prost::Message;
use time::OffsetDateTime;
use tracing::Subscriber;
use tracing_core::Event;
use tracing_subscriber::{layer::Context, Layer};
use worker::{console_debug, console_error, console_log, Cf};

use tracing::field::{Field, Visit};

use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
use opentelemetry_proto::tonic::resource::v1::Resource;

use crate::USER_AGENT;

/// Convert a batch of log records into a byte array and content type
fn build_logs_export_body(logs: Vec<LogRecord>, cf: &Cf) -> ExportLogsServiceRequest {
    let scope_logs = ScopeLogs {
        scope: None,
        log_records: logs,
        schema_url: "".to_string(),
    };

    let region = cf.region().map(|region| AnyValue {
        value: Some(any_value::Value::StringValue(region)),
    });
    let zone = Some(AnyValue {
        value: Some(any_value::Value::StringValue(cf.colo())),
    });

    let resource_logs = ResourceLogs {
        resource: Some(Resource {
            attributes: vec![
                KeyValue {
                    key: "service.name".to_string(),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue("pyoci".to_string())),
                    }),
                },
                KeyValue {
                    key: "cloud.region".to_string(),
                    value: region,
                },
                KeyValue {
                    key: "cloud.availability_zone".to_string(),
                    value: zone,
                },
            ],
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
    cf: &Cf,
) {
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
            Err(_) => break,
        };
        let body = build_logs_export_body(log_records, cf).encode_to_vec();
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
                    console_error!("Failed to send logs to OTLP: {:?}", response);
                } else {
                    console_debug!("Logs sent to OTLP: {:?}", response);
                };
            }
            Err(err) => {
                console_error!("Failed to send logs to OTLP: {:?}", err);
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
    fn flush(&self) -> Result<()> {
        let records: Vec<LogRecord> = self.records.write().unwrap().drain(..).collect();
        console_log!("Sending {} log records to OTLP", records.len());
        self.sender.try_send(records)?;
        Ok(())
    }
}

impl<S: Subscriber> Layer<S> for OtlpLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = event.metadata().level();
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let Some(time_ns) = time_unix_ns() else {
            return;
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
            trace_id: vec![],
            span_id: vec![],
            flags: 0,
        };

        self.records.write().unwrap().push(log_record);
    }

    fn on_close(&self, _id: tracing_core::span::Id, _ctx: Context<'_, S>) {
        if let Err(err) = self.flush() {
            console_error!("Failed to flush log records: {:?}", err);
        }
    }
}

fn time_unix_ns() -> Option<u64> {
    match OffsetDateTime::now_utc().unix_timestamp_nanos().try_into() {
        Ok(value) => Some(value),
        Err(_) => {
            console_error!("SystemTime out of range for conversion to u64!");
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
