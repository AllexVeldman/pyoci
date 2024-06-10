use anyhow::{bail, Result};
use askama::Template;
use std::str::FromStr;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::MakeWebConsoleWriter;
use worker::{
    event, Context, Env, FormEntry, Request, Response, ResponseBuilder, RouteContext, Router,
};

use crate::{package, templates, PyOci};

/// Wrap a async route handler into a closure that can be used in the router.
///
/// Allows request handlers to return Result<Response, pyoci::Error> instead of worker::Result<worker::Response>
#[macro_export]
macro_rules! wrap {
    ($e:expr) => {
        |req: Request, ctx: RouteContext<()>| async { wrap($e(req, ctx).await) }
    };
}

fn wrap(res: Result<Response>) -> worker::Result<Response> {
    match res {
        Ok(response) => Ok(response),
        Err(e) => Response::error(e.to_string(), 400),
    }
}

/// Called once when the worker is started
#[event(start)]
fn start() {
    // Ensure panics are logged to the worker console
    console_error_panic_hook::set_once();

    // Setup tracing
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .with_timer(UtcTime::rfc_3339())
        .with_writer(MakeWebConsoleWriter::new())
        .with_filter(LevelFilter::INFO);

    tracing_subscriber::registry().with(fmt_layer).init();
}

/// Called for each request to the worker
#[tracing::instrument(skip(req, env, _ctx), fields(path = %req.path(), method = %req.method()))]
#[event(fetch, respond_with_errors)]
async fn main(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
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
