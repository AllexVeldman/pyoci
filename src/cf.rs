use anyhow::{bail, Result};
use askama::Template;
use opentelemetry_proto::tonic::logs::v1::LogRecord;
use std::{str::FromStr, sync::OnceLock};
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;
use tracing_web::MakeWebConsoleWriter;
use worker::{
    console_log, event, Context, Env, FormEntry, Request, Response, ResponseBuilder, RouteContext,
    Router,
};

use crate::{package, pyoci::OciError, templates, PyOci};

/// Wrap a async route handler into a closure that can be used in the router.
///
/// Allows request handlers to return Result<Response, pyoci::Error> instead of worker::Result<worker::Response>
macro_rules! wrap {
    ($e:expr) => {
        |req: Request, ctx: RouteContext<()>| async { wrap($e(req, ctx).await) }
    };
}

fn wrap(res: Result<Response>) -> worker::Result<Response> {
    match res {
        Ok(response) => Ok(response),
        Err(e) => match e.downcast_ref::<OciError>() {
            Some(err) => Response::error(err.to_string(), err.status().into()),
            None => Response::error(e.to_string(), 400),
        },
    }
}

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
async fn fetch(req: Request, env: Env, ctx: Context) -> worker::Result<Response> {
    let receiver = init(&env);
    let cf = req.cf().expect("valid cf").clone();
    let otlp_endpoint = match env.secret("OTLP_ENDPOINT") {
        Ok(endpoint) => endpoint.to_string(),
        Err(_) => "".to_string(),
    };
    let otlp_auth = match env.secret("OTLP_AUTH") {
        Ok(auth) => auth.to_string(),
        Err(_) => "".to_string(),
    };

    let result = _fetch(req, env, ctx).await;

    if let Some(receiver) = receiver {
        crate::otlp::flush(receiver, otlp_endpoint, otlp_auth, &cf).await;
    }
    result
}

#[tracing::instrument(
    name="fetch",
    skip(req, env, _ctx),
    fields(path = %req.path(), method = %req.method()))
]
async fn _fetch(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    let method = req.method().to_string();
    let path = req.path();

    let response = router().run(req, env).await;

    let status = match &response {
        Ok(response) => response.status_code(),
        Err(_) => 400,
    };

    tracing::info!(method, status, path, "type" = "request");
    response
}

/// Request Router
fn router<'a>() -> Router<'a, ()> {
    Router::new()
        .get_async("/:registry/:namespace/:package/", wrap!(list_package))
        .get_async(
            "/:registry/:namespace/:package/:filename",
            wrap!(download_package),
        )
        .post_async("/:registry/:namespace/", wrap!(publish_package))
}

/// List package request handler
async fn list_package(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let auth = req.headers().get("Authorization").expect("valid header");
    let package = package::Info::from_str(&req.path())?;
    let client = PyOci::new(package.registry.clone(), auth);
    // Fetch at most 45 packages
    // https://developers.cloudflare.com/workers/platform/limits/#account-plan-limits
    let files = client.list_package_files(&package, 45).await?;
    let mut host = req.url().expect("valid url");
    host.set_path("");
    // TODO: swap to application/vnd.pypi.simple.v1+json
    let template = templates::ListPackageTemplate { host, files };
    Ok(
        Response::from_html(template.render().expect("valid template"))
            .expect("valid html response"),
    )
}

/// Download package request handler
async fn download_package(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let auth = req.headers().get("Authorization").expect("valid header");
    let package = package::Info::from_str(&req.path())?;
    let client = PyOci::new(package.registry.clone(), auth);
    let data = client
        .download_package_file(&package)
        .await?
        .bytes()
        .await
        .expect("valid bytes");

    // TODO: With some trickery we could stream the data directly to the response
    let response = ResponseBuilder::new()
        .with_header(
            "Content-Disposition",
            &format!("attachment; filename=\"{}\"", package.filename()),
        )
        .expect("valid header")
        .from_bytes(data.into())
        .expect("valid response");
    Ok(response)
}

/// Publish package request handler
///
/// ref: https://warehouse.pypa.io/api-reference/legacy.html#upload-api
async fn publish_package(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (Some(registry), Some(namespace)) = (ctx.param("registry"), ctx.param("namespace")) else {
        bail!("Missing registry or namespace");
    };
    let Ok(form_data) = req.form_data().await else {
        bail!("Invalid form data");
    };
    let Some(content) = form_data.get("content") else {
        bail!("Missing file");
    };
    let FormEntry::File(file) = content else {
        bail!("Expected file");
    };
    let auth = req.headers().get("Authorization").expect("valid header");
    let package = package::Info::new(registry, namespace, &file.name())?;
    let client = PyOci::new(package.registry.clone(), auth);

    // FormEntry::File does not provide a streaming interface
    // so we must read the entire file into memory
    let data = file.bytes().await.expect("valid bytes");

    client.publish_package_file(&package, data).await?;
    Ok(Response::ok("Published").unwrap())
}
