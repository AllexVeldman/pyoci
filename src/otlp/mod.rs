mod log;
mod trace;

pub use log::OtlpLogLayer;
pub use trace::OtlpTraceLayer;
pub use trace::SpanTimeLayer;

pub type OtlpLayer = (Option<OtlpLogLayer>, Option<OtlpTraceLayer>);
