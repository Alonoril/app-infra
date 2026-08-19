use crate::{middleware::HTTP_TIMEOUT, resp::WebErr, status_res};
use axum::{BoxError, response::IntoResponse};
use http::StatusCode;

/// Adds a custom handler for tower's `TimeoutLayer`, see https://docs.rs/axum/latest/axum/middleware/index.html#commonly-used-middleware.
pub async fn handle_timeout_error(err: BoxError) -> impl IntoResponse {
	if err.is::<tower::timeout::error::Elapsed>() {
		status_res!(
			StatusCode::REQUEST_TIMEOUT,
			WebErr::RequestTimeout,
			format_args!("request took longer than the {HTTP_TIMEOUT} second timeout"),
		)
	} else {
		status_res!(
			StatusCode::INTERNAL_SERVER_ERROR,
			WebErr::InternalServerError,
			format_args!("unhandled internal error: {err}"),
		)
	}
}

pub async fn handle_404() -> impl IntoResponse {
	status_res!(StatusCode::NOT_FOUND, WebErr::SourceNotFound)
}
