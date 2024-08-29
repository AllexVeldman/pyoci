use http::{Request, Response};
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

fn init(env: &Env) -> &'static crate::otlp::OtlpLayer {
    static INIT: OnceLock<crate::otlp::OtlpLayer> = OnceLock::new();
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

        let (log_layer, trace_layer) = if let (Ok(otlp_endpoint), Ok(otlp_auth)) =
            (env.secret("OTLP_ENDPOINT"), env.secret("OTLP_AUTH"))
        {
            let log_layer =
                crate::otlp::OtlpLogLayer::new(otlp_endpoint.to_string(), otlp_auth.to_string());
            let trace_layer =
                crate::otlp::OtlpTraceLayer::new(otlp_endpoint.to_string(), otlp_auth.to_string());
            (Some(log_layer), Some(trace_layer))
        } else {
            (None, None)
        };

        registry
            .with(crate::otlp::SpanTimeLayer::new())
            .with(
                trace_layer
                    .clone()
                    .with_filter(EnvFilter::new(rust_log.clone())),
            )
            .with(log_layer.clone().with_filter(EnvFilter::new(rust_log)))
            .init();
        console_log!("Worker initialized");
        // TODO: combine otlp layer so we can flush as one.
        (log_layer, trace_layer)
    })
}

/// Entrypoint for the fetch event
#[event(fetch, respond_with_errors)]
async fn fetch(
    req: Request<Body>,
    env: Env,
    ctx: Context,
) -> worker::Result<Response<axum::body::Body>> {
    let otlp_layer = init(&env);
    let cf = req.extensions().get::<Cf>().unwrap().to_owned();

    let span = info_span!("fetch", path = %req.uri().path(), method = %req.method());
    let result = crate::app::router().call(req).instrument(span).await;
    if let (Some(log_layer), Some(trace_layer)) = otlp_layer {
        let attributes = HashMap::from([
            ("service.name", Some("pyoci".to_string())),
            ("cloud.region", cf.region()),
            ("cloud.availability_zone", Some(cf.colo())),
        ]);
        ctx.wait_until(async move {
            log_layer.flush(&attributes).await;
            trace_layer.flush(&attributes).await;
        });
    }
    Ok(result?)
}
