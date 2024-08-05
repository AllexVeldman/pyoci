use http::{Request, Response};
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use std::collections::HashMap;
use std::sync::OnceLock;
use tower::Service;
use tracing::{info_span, Instrument};
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;
use tracing_web::MakeWebConsoleWriter;
use worker::{console_log, event, Body, Cf, Context, Env};

/// Called once when the worker is started
#[event(start)]
fn start() {
    // Ensure panics are logged to the worker console
    console_error_panic_hook::set_once();
}

fn init(env: &Env) -> &'static Option<async_channel::Receiver<Vec<LogRecord>>> {
    static INIT: OnceLock<Option<async_channel::Receiver<Vec<LogRecord>>>> = OnceLock::new();
    INIT.get_or_init(|| {
        let rust_log = match env.var("RUST_LOG") {
            Ok(log) => log.to_string(),
            Err(_) => "info".to_string(),
        };

        // Setup tracing
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_timer(UtcTime::rfc_3339())
            .with_writer(MakeWebConsoleWriter::new())
            .with_filter(EnvFilter::new(&rust_log));

        let registry = tracing_subscriber::registry().with(fmt_layer);
        // OTLP exporter
        let (otlp_layer, receiver) = if env.secret("OTLP_ENDPOINT").is_err() {
            (None, None)
        } else {
            let (otlp_layer, receiver) = crate::otlp::OtlpLogLayer::new();
            (
                Some(otlp_layer.with_filter(EnvFilter::new(rust_log))),
                Some(receiver),
            )
        };

        registry.with(otlp_layer).init();
        console_log!("Worker initialized");
        receiver
    })
}

/// Entrypoint for the fetch event
#[event(fetch, respond_with_errors)]
async fn fetch(
    req: Request<Body>,
    env: Env,
    ctx: Context,
) -> worker::Result<Response<axum::body::Body>> {
    let receiver = init(&env);
    let cf = req.extensions().get::<Cf>().unwrap().to_owned();
    let otlp_endpoint = match env.secret("OTLP_ENDPOINT") {
        Ok(endpoint) => endpoint.to_string(),
        Err(_) => "".to_string(),
    };
    let otlp_auth = match env.secret("OTLP_AUTH") {
        Ok(auth) => auth.to_string(),
        Err(_) => "".to_string(),
    };

    let span = info_span!("fetch", path = %req.uri().path(), method = %req.method());
    let result = crate::app::router().call(req).instrument(span).await;

    if let Some(receiver) = receiver {
        let attributes = HashMap::from([
            ("service.name".to_string(), Some("pyoci".to_string())),
            ("cloud.region".to_string(), cf.region()),
            ("cloud.availability_zone".to_string(), Some(cf.colo())),
        ]);
        ctx.wait_until(async move {
            crate::otlp::flush(receiver, otlp_endpoint, otlp_auth, &attributes).await;
        });
    }
    Ok(result?)
}
