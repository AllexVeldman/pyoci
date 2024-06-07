use askama::Template;
use std::str::FromStr;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::MakeWebConsoleWriter;
use worker::{
    event, Context, Env, FormEntry, Request, Response, ResponseBuilder, RouteContext, Router,
};

use crate::{package, pyoci, templates, PyOci};

/// Wrap a async route handler into a closure that can be used in the router.
///
/// Allows request handlers to return Result<Response, pyoci::Error> instead of worker::Result<worker::Response>
#[macro_export]
macro_rules! wrap {
    ($e:expr) => {
        |req: Request, ctx: RouteContext<()>| async { wrap($e(req, ctx).await) }
    };
}

fn wrap(res: Result<Response, pyoci::Error>) -> worker::Result<Response> {
    let err = match res {
        Ok(response) => return Ok(response),
        Err(e) => e,
    };
    match err {
        pyoci::Error::OciErrorResponse(resp) => {
            Response::error(serde_json::to_string(&resp).expect("valid json"), 400)
        }
        err => Response::error(err.to_string(), 400),
    }
}

/// Called once when the worker is started
#[event(start)]
fn start() {
    // Ensure panics are logged to the worker console
    console_error_panic_hook::set_once();

    // OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter().http();

    // Setup tracing
    let console_log_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .with_timer(UtcTime::rfc_3339())
        .with_writer(MakeWebConsoleWriter::new())
        .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry()
        .with(console_log_layer)
        .init();
}

/// Called for each request to the worker
#[tracing::instrument(skip(req, env, _ctx), fields(path = %req.path(), method = %req.method()))]
#[event(fetch, respond_with_errors)]
async fn main(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    tracing::debug!("Request Headers: {:#?}", req.headers());
    router().run(req, env).await
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
async fn list_package(req: Request, _ctx: RouteContext<()>) -> Result<Response, pyoci::Error> {
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
async fn download_package(req: Request, _ctx: RouteContext<()>) -> Result<Response, pyoci::Error> {
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
            &format!("attachment; filename=\"{}\"", package.file),
        )
        .expect("valid header")
        .from_bytes(data.into())
        .expect("valid response");
    Ok(response)
}

/// Publish package request handler
///
/// ref: https://warehouse.pypa.io/api-reference/legacy.html#upload-api
async fn publish_package(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response, pyoci::Error> {
    let (Some(registry), Some(namespace)) = (ctx.param("registry"), ctx.param("namespace")) else {
        return Err(pyoci::Error::Other(
            "Missing registry or namespace".to_string(),
        ));
    };
    let Ok(form_data) = req.form_data().await else {
        return Err(pyoci::Error::Other("Invalid form data".to_string()));
    };
    let Some(content) = form_data.get("content") else {
        return Err(pyoci::Error::Other("Missing file".to_string()));
    };
    let FormEntry::File(file) = content else {
        return Err(pyoci::Error::Other("Expected file".to_string()));
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
