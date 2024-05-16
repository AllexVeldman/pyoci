use askama::Template;
use base64::prelude::{Engine as _, BASE64_STANDARD};
use pyoci::{client, client::OciTransport, package};
use std::str::FromStr;
use tracing_subscriber::fmt::format::Pretty;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_web::{performance_layer, MakeWebConsoleWriter};
use worker::{console_log, event, Context, Env, Request, Response, Result, RouteContext, Router};

mod templates;
mod transport;

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

#[event(fetch, respond_with_errors)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    router().run(req, env).await
}

fn router<'a>() -> Router<'a, ()> {
    Router::new()
        .get_async("/:registry/:namespace/:package", list_package)
        .get_async("/api/:id", api)
}

#[tracing::instrument(skip(req, _ctx))]
async fn list_package(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let (username, password) = parse_auth(
        &req.headers()
            .get("Authorization")
            .expect("valid header")
            .unwrap_or("".to_string()),
    );
    let package = package::Info::from_str(&req.path()).expect("valid package");
    let transport =
        transport::JsTransport::new(package.registry.clone()).with_auth(username, password);
    let client = client::Client::new(transport);
    let files = client
        .list_package_files(&package)
        .await
        .expect("valid files");
    let mut host = req.url().expect("valid url");
    host.set_path("");
    let template = templates::ListPackageTemplate { host, files };
    Response::ok(template.render().expect("valid template"))
}

async fn api(_req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    console_log!("ID: {:?}", _ctx.param("id"));
    let sum = add(1, 2);
    Response::ok(format!("Hello, World: {}", sum))
}

fn parse_auth(value: &str) -> (Option<String>, Option<String>) {
    tracing::debug!("Parsing auth header: {:?}", value);
    let Some(value) = value.strip_prefix("Basic ") else {
        return (None, None);
    };
    match BASE64_STANDARD.decode(value.as_bytes()) {
        Ok(decoded) => {
            let decoded = String::from_utf8(decoded).expect("valid utf8");
            match decoded.splitn(2, ':').collect::<Vec<&str>>()[..] {
                [username, password] => (Some(username.to_string()), Some(password.to_string())),
                _ => (None, None),
            }
        }
        Err(err) => {
            tracing::warn!("Failed to decode auth header: {:?}", err);
            (None, None)
        }
    }
}

// Include JS snippets
// rust-analyser will complain about these calls, but they are valid
// #[wasm_bindgen(module = "/js/foo.js")]
// extern "C" {
//     fn add(a: u32, b: u32) -> u32;
// }
