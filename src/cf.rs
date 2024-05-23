use askama::Template;
use std::str::FromStr;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use worker::{event, Context, Env, Request, Response, ResponseBuilder, RouteContext, Router};

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
        .with_writer(MakeWebConsoleWriter::new());
    let perf_layer = performance_layer().with_details_from_fields(Pretty::default());

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(perf_layer)
        .init();
}

/// Called for each request to the worker
#[event(fetch, respond_with_errors)]
async fn main(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    router().run(req, env).await
}

/// Request Router
fn router<'a>() -> Router<'a, ()> {
    Router::new()
        .get_async("/:registry/:namespace/:package", wrap!(list_package))
        .get_async(
            "/:registry/:namespace/:package/:filename",
            wrap!(download_package),
        )
}

/// List package request handler
#[tracing::instrument(skip(req, _ctx))]
async fn list_package(req: Request, _ctx: RouteContext<()>) -> Result<Response, pyoci::Error> {
    let auth = req.headers().get("Authorization").expect("valid header");
    let package = package::Info::from_str(&req.path())?;
    let client = PyOci::new(package.registry.clone(), auth);
    let files = client
        .list_package_files(&package)
        .await
        .expect("valid files");
    let mut host = req.url().expect("valid url");
    host.set_path("");
    let template = templates::ListPackageTemplate { host, files };
    Ok(Response::ok(template.render().expect("valid template")).expect("valid response"))
}

/// Download package request handler
#[tracing::instrument(skip(req, _ctx))]
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
