mod api_tracing;
mod err_wraper;

use http::Request;
use infra_core::utils::UID;
use tracing::{Span, info, info_span};

pub use api_tracing::http_trace;
pub use err_wraper::{handle_404, handle_timeout_error};

pub const HTTP_TIMEOUT: u64 = 10;
pub const EXPONENTIAL_SECONDS: &[f64] = &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

pub fn make_span<B>(request: &Request<B>) -> Span {
	let trace_id = UID.trace_id();
	info_span!(
		"api",
		tid = %trace_id,
		method = %request.method(),
		path = %request.uri().path(),
	)
}

pub fn accept_trace<B>(request: Request<B>) -> Request<B> {
	request
}

pub fn record_trace_id<B>(request: Request<B>) -> Request<B> {
	info!(uri = %request.uri());
	request
}
