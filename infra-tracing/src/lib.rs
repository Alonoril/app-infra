//! Tracing initialization and rolling log appenders.

pub mod appender;
mod logger;
#[cfg(feature = "test-utils")]
pub mod tests;

pub use logger::{Logger, LoggerGuard};
pub use tracing_appender::non_blocking::WorkerGuard;
