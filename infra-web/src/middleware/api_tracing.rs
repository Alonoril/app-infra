use axum::{extract::Request, middleware::Next, response::Response};
use http::{
	HeaderMap,
	header::{CONTENT_LENGTH, HeaderValue, USER_AGENT},
};
use infra_core::utils::UID;
use std::time::Instant;
use tracing::{Instrument, info, info_span};

const REQUEST_ID_HEADER: &str = "request-id";
const API_PATH_PREFIXES: &[&str] = &["/api/", "/v1/", "/v2/", "/v3/"];

#[inline]
fn should_trace_path(path: &str) -> bool {
	API_PATH_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

#[inline]
fn header_to_str<'a>(headers: &'a HeaderMap, name: &'static http::header::HeaderName) -> Option<&'a str> {
	headers.get(name).and_then(|value| value.to_str().ok())
}

#[inline]
fn forwarded_addr(headers: &HeaderMap) -> Option<&str> {
	headers
		.get("x-forwarded-for")
		.or_else(|| headers.get("x-real-ip"))
		.and_then(|value| value.to_str().ok())
}

pub async fn http_trace(req: Request, next: Next) -> Response {
	if !should_trace_path(req.uri().path()) {
		return next.run(req).await;
	}

	let request_id = UID.trace_id();
	let started_at = Instant::now();
	let span = info_span!(
		"api",
		tid = %request_id,
		method = %req.method(),
		path = %req.uri().path(),
	);

	async move {
		info!(
			target: "http_request",
			user_agent = header_to_str(req.headers(), &USER_AGENT),
			remote_addr = forwarded_addr(req.headers()),
			content_length = header_to_str(req.headers(), &CONTENT_LENGTH),
			"Request started"
		);

		let mut response = next.run(req).await;
		let status_code = response.status().as_u16();
		let duration_ms = started_at.elapsed().as_millis();

		let mut request_id_buffer = [0; 32];
		match HeaderValue::from_str(request_id.encode_lower(&mut request_id_buffer)) {
			Ok(value) => {
				response.headers_mut().insert(REQUEST_ID_HEADER, value);
			}
			Err(err) => {
				tracing::warn!(
					target: "http_request",
					reason = ?err,
					"failed to set request id header"
				);
			}
		}

		info!(
			target: "http_request",
			status_code,
			duration_ms,
			"Request completed"
		);

		response
	}
	.instrument(span)
	.await
}
