use crate::resp::{RespData, WebErr};
use axum::{
	Json,
	extract::rejection::{JsonRejection, QueryRejection},
	http::StatusCode,
	response::{IntoResponse, Response},
};
use infra_core::result::{AppError, ErrCodeTrait};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};
use utoipa::ToSchema;

const HTTP_STATUS_MODE_ALWAYS_OK: u8 = 0;
const HTTP_STATUS_MODE_STATUS_CODE: u8 = 1;

static HTTP_STATUS_MODE: AtomicU8 = AtomicU8::new(HTTP_STATUS_MODE_ALWAYS_OK);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpStatusMode {
	#[default]
	AlwaysOk,
	StatusCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebRespConfig {
	#[serde(default)]
	pub http_status_mode: HttpStatusMode,
}

impl Default for WebRespConfig {
	fn default() -> Self {
		Self {
			http_status_mode: HttpStatusMode::AlwaysOk,
		}
	}
}

pub fn set_http_status_mode(mode: HttpStatusMode) {
	let mode = match mode {
		HttpStatusMode::AlwaysOk => HTTP_STATUS_MODE_ALWAYS_OK,
		HttpStatusMode::StatusCode => HTTP_STATUS_MODE_STATUS_CODE,
	};
	HTTP_STATUS_MODE.store(mode, Ordering::Relaxed);
}

pub fn http_status_mode() -> HttpStatusMode {
	match HTTP_STATUS_MODE.load(Ordering::Relaxed) {
		HTTP_STATUS_MODE_STATUS_CODE => HttpStatusMode::StatusCode,
		_ => HttpStatusMode::AlwaysOk,
	}
}

#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct StatusAppError {
	pub status: StatusCode,
	pub error: AppError,
}

impl StatusAppError {
	pub const fn new(status: StatusCode, error: AppError) -> Self {
		Self { status, error }
	}

	pub fn from_code<C>(status: StatusCode, code: C) -> Self
	where
		C: Into<AppError>,
	{
		Self::new(status, code.into())
	}

	pub fn from_code_msg<C, M>(status: StatusCode, code: C, message: M) -> Self
	where
		C: ErrCodeTrait,
		M: Into<String>,
	{
		Self::new(status, AppError::from_code_msg(code, message))
	}

	pub fn bad_request(error: AppError) -> Self {
		Self::new(StatusCode::BAD_REQUEST, error)
	}

	pub fn unauthorized(error: AppError) -> Self {
		Self::new(StatusCode::UNAUTHORIZED, error)
	}

	pub fn forbidden(error: AppError) -> Self {
		Self::new(StatusCode::FORBIDDEN, error)
	}

	pub fn not_found(error: AppError) -> Self {
		Self::new(StatusCode::NOT_FOUND, error)
	}

	pub fn conflict(error: AppError) -> Self {
		Self::new(StatusCode::CONFLICT, error)
	}

	pub fn unprocessable_entity(error: AppError) -> Self {
		Self::new(StatusCode::UNPROCESSABLE_ENTITY, error)
	}

	pub fn internal_server_error(error: AppError) -> Self {
		Self::new(StatusCode::INTERNAL_SERVER_ERROR, error)
	}
}

impl<C> From<(StatusCode, C)> for StatusAppError
where
	C: Into<AppError>,
{
	fn from((status, code): (StatusCode, C)) -> Self {
		Self::from_code(status, code)
	}
}

#[derive(Debug, thiserror::Error)]
pub enum AxumError {
	#[error("Request JSON error : {0}")]
	AxumJson(#[from] JsonRejection),
	#[error("Invalid query: {0}")]
	AxumParams(#[from] QueryRejection),
	#[error("{0}")]
	AppError(#[from] AppError),
	#[error("{0}")]
	StatusAppError(#[from] StatusAppError),
}

impl<C> From<(StatusCode, C)> for AxumError
where
	C: Into<AppError>,
{
	fn from(value: (StatusCode, C)) -> Self {
		Self::StatusAppError(value.into())
	}
}

#[derive(axum_macros::FromRequest)]
#[from_request(via(axum::Json), rejection(AxumError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
	Json<T>: IntoResponse,
{
	fn into_response(self) -> Response {
		Json(self.0).into_response()
	}
}

#[derive(axum_macros::FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AxumError))]
pub struct Query<T>(pub T);

impl IntoResponse for AxumError {
	fn into_response(self) -> Response {
		match self {
			AxumError::AxumJson(err) => {
				let web_err = WebErr::AxumRequestJson;
				tracing::error!(domain_code = web_err.domain_code(), reason = ?err);
				let resp = RespData::with_anyhow(web_err, err.into());
				status_response(StatusCode::BAD_REQUEST, resp)
			}
			AxumError::AxumParams(err) => {
				let web_err = WebErr::AxumQueryParams;
				tracing::error!(domain_code = web_err.domain_code(), reason = ?err);
				let resp = RespData::with_anyhow(web_err, err.into());
				status_response(StatusCode::BAD_REQUEST, resp)
			}
			AxumError::AppError(err) => {
				let resp = RespData::with_app_error(err);
				status_response(StatusCode::INTERNAL_SERVER_ERROR, resp)
			}
			AxumError::StatusAppError(err) => {
				let resp = RespData::with_app_error(err.error);
				status_response(err.status, resp)
			}
		}
	}
}

pub fn status_response(status: StatusCode, resp: RespData) -> Response {
	let status = match http_status_mode() {
		HttpStatusMode::AlwaysOk => StatusCode::OK,
		HttpStatusMode::StatusCode => status,
	};
	(status, AppJson(resp)).into_response()
}

pub type AxumResult<T> = Result<T, AxumError>;

#[cfg(test)]
pub(crate) mod status_mode_test {
	use super::{HttpStatusMode, http_status_mode, set_http_status_mode};
	use std::sync::{Mutex, MutexGuard};

	static STATUS_MODE_LOCK: Mutex<()> = Mutex::new(());

	pub(crate) struct HttpStatusModeGuard {
		_guard: MutexGuard<'static, ()>,
		previous: HttpStatusMode,
	}

	impl Drop for HttpStatusModeGuard {
		fn drop(&mut self) {
			set_http_status_mode(self.previous);
		}
	}

	pub(crate) fn set_for_test(mode: HttpStatusMode) -> HttpStatusModeGuard {
		let guard = STATUS_MODE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
		let previous = http_status_mode();
		set_http_status_mode(mode);
		HttpStatusModeGuard {
			_guard: guard,
			previous,
		}
	}
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AxumResp<T: ToSchema> {
	code: String,
	msg: String,
	data: Option<T>,
}

impl<T> From<RespData<T>> for AxumResp<T>
where
	T: ToSchema,
{
	fn from(value: RespData<T>) -> Self {
		Self {
			code: value.code.to_string(),
			msg: value.msg.into_owned(),
			data: value.data,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{AxumError, HttpStatusMode, StatusAppError, http_status_mode, status_mode_test};
	use axum::{http::StatusCode, response::IntoResponse};
	use infra_core::result::{AppError, SysErr};

	#[test]
	fn defaults_to_always_ok_mode() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::AlwaysOk);

		assert_eq!(http_status_mode(), HttpStatusMode::AlwaysOk);
	}

	#[test]
	fn app_error_uses_ok_in_compat_mode() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::AlwaysOk);

		let resp = AxumError::from(AppError::from_code(SysErr::InvalidParams)).into_response();

		assert_eq!(resp.status(), StatusCode::OK);
	}

	#[test]
	fn status_app_error_uses_ok_in_compat_mode() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::AlwaysOk);

		let resp =
			AxumError::from(StatusAppError::bad_request(AppError::from_code(SysErr::InvalidParams))).into_response();

		assert_eq!(resp.status(), StatusCode::OK);
	}

	#[test]
	fn status_code_mode_uses_default_app_error_status() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::StatusCode);

		let resp = AxumError::from(AppError::from_code(SysErr::InvalidParams)).into_response();

		assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
	}

	#[test]
	fn status_code_mode_uses_status_app_error_status() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::StatusCode);

		let resp =
			AxumError::from(StatusAppError::not_found(AppError::from_code(SysErr::InvalidParams))).into_response();

		assert_eq!(resp.status(), StatusCode::NOT_FOUND);
	}

	#[test]
	fn status_code_tuple_converts_to_status_app_error() {
		let _guard = status_mode_test::set_for_test(HttpStatusMode::StatusCode);

		let resp = AxumError::from((StatusCode::CONFLICT, SysErr::InvalidParams)).into_response();

		assert_eq!(resp.status(), StatusCode::CONFLICT);
	}

	#[test]
	fn status_app_error_from_code_msg_uses_extended_message() {
		let err = StatusAppError::from_code_msg(StatusCode::BAD_REQUEST, SysErr::InvalidParams, "missing name");

		assert_eq!(err.status, StatusCode::BAD_REQUEST);
		assert_eq!(err.error.message(), "Invalid params: missing name");
	}
}
