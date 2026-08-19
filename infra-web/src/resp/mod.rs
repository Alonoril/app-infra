mod axum;
mod resp_data;
pub use axum::{
	AppJson, AxumError, AxumResp, AxumResult, HttpStatusMode, Query, StatusAppError, WebRespConfig, http_status_mode,
	set_http_status_mode, status_response,
};
pub use resp_data::{RespCode, RespData};

/// Ok(AppJson(RespData::success(admin)))
#[macro_export]
macro_rules! success {
	($data:expr) => {{
		tracing::debug!(response_data=?$data);
		Ok($crate::resp::AppJson($crate::resp::RespData::success($data)))
	}};
}

/// Build a response with an HTTP status candidate and a web error code.
#[macro_export]
macro_rules! status_res {
	($status:expr, $code:expr $(,)?) => {{ $crate::resp::status_response($status, $crate::resp::RespData::with_code($code)) }};

	($status:expr, $code:expr, $msg:expr $(,)?) => {{ $crate::resp::status_response($status, $crate::resp::RespData::with_display_msg($code, $msg)) }};
}

/// use this macro to return Err(AxumError) with an HTTP status candidate
///
/// The global HttpStatusMode decides whether the candidate status is emitted
/// or collapsed to 200 OK.
#[macro_export]
macro_rules! status_err {
	($status:expr, $code:expr) => {{
		tracing::error!(status = %$status, err = %$code);
		Err($crate::resp::AxumError::from(($status, $code)))
	}};

	($status:expr, $code:expr, $msg:expr) => {{
		tracing::error!(status = %$status, err = %$code, msg = %$msg);
		Err($crate::resp::AxumError::from($crate::resp::StatusAppError::from_code_msg(
			$status,
			$code,
			$msg,
		)))
	}};
}

infra_core::define_app_error_codes! {
	WebErr("WEB") {
		SourceNotFound = (100, "Resource not found"),
		AxumServerError = (101, "Axum server error"),
		AxumRequestJson = (102, "Error in the json payload"),
		AxumQueryParams = (103, "Error in the query params"),

		MissingExtension = (104, "Axum server error: MissingExtension"),
		UserAgentNotFound = (105, "User-Agent header not found in request"),
		RequestTimeout = (106, "Request timeout"),
		InternalServerError = (107, "unhandled internal error"),
		AxumPathParams = (108, "Error in the path params"),
	}
}

#[cfg(test)]
mod tests {
	use super::{AxumError, HttpStatusMode, WebErr};
	use crate::resp::axum::status_mode_test;
	use axum::{http::StatusCode, response::IntoResponse};

	#[test]
	fn status_err_macro_returns_axum_error_with_status() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::StatusCode);

		let result: Result<(), AxumError> = crate::status_err!(StatusCode::NOT_FOUND, WebErr::SourceNotFound);
		let resp = result.expect_err("status_err should return Err").into_response();

		assert_eq!(resp.status(), StatusCode::NOT_FOUND);
	}

	#[test]
	fn status_err_macro_supports_extended_message() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::StatusCode);

		let result: Result<(), AxumError> =
			crate::status_err!(StatusCode::BAD_REQUEST, WebErr::AxumQueryParams, "missing page");
		let resp = result.expect_err("status_err should return Err").into_response();

		assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
	}
}
