use askama::Template;
use std::str::FromStr;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
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

    // Setup tracing
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .with_timer(UtcTime::rfc_3339())
        .with_writer(MakeWebConsoleWriter::new())
        .with_filter(LevelFilter::DEBUG);
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
        .get_async("/:registry/:namespace/:package/", wrap!(list_package))
        .get_async(
            "/:registry/:namespace/:package/:filename",
            wrap!(download_package),
        )
        .post_async("/:registry/:namespace", wrap!(publish_package))
}

/// List package request handler
#[tracing::instrument(skip(req, _ctx))]
async fn list_package(req: Request, _ctx: RouteContext<()>) -> Result<Response, pyoci::Error> {
    let auth = req.headers().get("Authorization").expect("valid header");
    let package = package::Info::from_str(&req.path())?;
    let client = PyOci::new(package.registry.clone(), auth);
    let files = client.list_package_files(&package).await?;
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

#[tracing::instrument(skip(req, ctx))]
async fn publish_package(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response, pyoci::Error> {
    let auth = req.headers().get("Authorization").expect("valid header");
    let Some(form_data) = req.form_data().await.expect("valid form data").get("file") else {
        return Err(pyoci::Error::Other("Missing file".to_string()));
    };
    let FormEntry::File(file) = form_data else {
        return Err(pyoci::Error::Other("Expected file".to_string()));
    };
    let (Some(registry), Some(namespace)) = (ctx.param("registry"), ctx.param("namespace")) else {
        return Err(pyoci::Error::Other(
            "Missing registry or namespace".to_string(),
        ));
    };
    let package = package::Info::new(registry, namespace, &file.name())?;
    let client = PyOci::new(package.registry.clone(), auth);

    let data = file.bytes().await.expect("valid bytes");

    client.publish_package_file(&package, data).await?;
    Ok(Response::ok("Published").unwrap())
}
