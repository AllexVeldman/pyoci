#![warn(unused_extern_crates)]

// Webserver request handlers
mod app;
// Request handlers for the cloudflare worker
#[cfg(target_arch = "wasm32")]
mod cf;
// OTLP handlers
#[cfg(feature = "otlp")]
mod otlp;
// Helper for parsing and managing Python/OCI packages
mod package;
// PyOci client
mod pyoci;
// Askama templates
mod templates;
// HTTP Transport
mod transport;
// Services
mod service;
// Re-export the PyOci client
pub use pyoci::PyOci;

// crate constants
const USER_AGENT: &str = concat!("pyoci ", env!("CARGO_PKG_VERSION"), " (cloudflare worker)");
const ARTIFACT_TYPE: &str = "application/pyoci.package.v1";
