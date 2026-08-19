//! Application error type and helper macros for converting failures into it.

use crate::result::ErrCodeTrait;
use std::borrow::Cow;

/// Structured application error carrying a stable domain/code pair and message.
///
/// `AppError` is the error type used by infrastructure code when a low-level
/// error needs to be surfaced through the application's public error contract.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum AppError {
	/// Error identified by a domain-specific numeric code.
	#[error("[{domain}:{code}] {message}")]
	ErrCode {
		/// Static domain prefix, for example `SYS`.
		domain: &'static str,
		/// Domain-local numeric error code.
		code: u16,
		/// Human-readable error message.
		message: Cow<'static, str>,
	},
}

impl AppError {
	/// Creates an [`AppError`] from explicit domain, code, and message values.
	pub fn new(domain: &'static str, code: u16, message: impl Into<Cow<'static, str>>) -> Self {
		Self::ErrCode {
			domain,
			code,
			message: message.into(),
		}
	}

	/// Creates an [`AppError`] from a typed error code.
	///
	/// The message is borrowed from the static message carried by the code.
	pub fn from_code<C: ErrCodeTrait>(code: C) -> Self {
		let code = code.err_code();

		Self::ErrCode {
			domain: code.domain,
			code: code.code,
			message: Cow::Borrowed(code.message),
		}
	}

	/// Creates an [`AppError`] from a typed error code plus extra context.
	///
	/// When `message` is empty, the code's static message is reused unchanged.
	/// Otherwise the final message is formatted as `"<static message>: <message>"`.
	pub fn from_code_msg<C, M>(code: C, message: M) -> Self
	where
		C: ErrCodeTrait,
		M: Into<String>,
	{
		let code = code.err_code();
		let extra_message = message.into();
		let message = if extra_message.is_empty() {
			Cow::Borrowed(code.message)
		} else {
			let mut message = String::with_capacity(code.message.len() + 2 + extra_message.len());
			message.push_str(code.message);
			message.push_str(": ");
			message.push_str(&extra_message);
			Cow::Owned(message)
		};

		Self::ErrCode {
			domain: code.domain,
			code: code.code,
			message,
		}
	}

	/// Returns the error domain.
	pub fn domain(&self) -> &'static str {
		match self {
			Self::ErrCode { domain, .. } => domain,
		}
	}

	/// Returns the numeric error code within the domain.
	pub fn code(&self) -> u16 {
		match self {
			Self::ErrCode { code, .. } => *code,
		}
	}

	/// Returns the human-readable error message.
	pub fn message(&self) -> &str {
		match self {
			Self::ErrCode { message, .. } => message,
		}
	}

	/// Returns a newly allocated `"<domain>:<code>"` identifier.
	pub fn domain_code(&self) -> String {
		match self {
			Self::ErrCode { domain, code, .. } => format!("{domain}:{code}"),
		}
	}

	/// Converts this error into an [`anyhow::Error`].
	pub fn into_anyhow(self) -> anyhow::Error {
		self.into()
	}
}

impl From<crate::fault::Fault> for AppError {
	fn from(value: crate::fault::Fault) -> Self {
		Self::new(
			"FLT",
			value.code_id(),
			format!(
				"domain={} detail={} raw={}",
				value.domain(),
				value.detail(),
				value.raw()
			),
		)
	}
}

impl From<crate::fault::FaultReport> for AppError {
	fn from(value: crate::fault::FaultReport) -> Self {
		let (fault, message) = value.into_parts();

		Self::ErrCode {
			domain: "FLT",
			code: fault.code_id(),
			message,
		}
	}
}

/// Builds a closure for [`Result::map_err`] that logs and maps a source error.
///
/// The macro accepts either a typed error code or a reference to one. The
/// two-argument form appends additional message context via
/// [`AppError::from_code_msg`].
///
/// Logging behavior:
/// - emits the stable `domain_code` at `error` level;
/// - emits the typed message and source error at `debug` level.
///
/// # Examples
///
/// ```rust,ignore
/// let value = fallible_call().map_err(map_err!(SysErr::ConfigLoadFailed))?;
/// let value = fallible_call().map_err(map_err!(SysErr::InvalidParams, "missing id"))?;
/// ```
#[macro_export]
macro_rules! map_err {
    (&$code:expr) => {
        $crate::map_err!($code)
    };
    (&$code:expr, $message:expr) => {
        $crate::map_err!($code, $message)
    };
    ($code:expr) => {{
        move |err| {
			use $crate::result::ErrCodeTrait;
            let code = $code;

            tracing::error!(code = code.domain_code());
            tracing::debug!(message = code.message(), error = ?err);

            $crate::result::AppError::from_code(code)
        }
    }};
    ($code:expr, $message:expr) => {{
        move |err| {
			use $crate::result::ErrCodeTrait;
            let code = $code;

            tracing::error!(code = code.domain_code());
            tracing::debug!(message = code.message(), error = ?err);

            $crate::result::AppError::from_code_msg(code, $message)
        }
    }};
}

/// Builds a closure for [`Result::map_err`] that logs source errors at error level.
///
/// This is the noisier variant of [`map_err!`]. It records `domain_code`,
/// message, and the debug representation of the source error in the same
/// `tracing::error!` event before returning an [`AppError`].
///
/// The macro accepts either a typed error code or a reference to one. The
/// two-argument form appends additional message context via
/// [`AppError::from_code_msg`].
///
/// # Examples
///
/// ```rust,ignore
/// let cfg = load_cfg().map_err(map_err_logged!(SysErr::ConfigLoadFailed))?;
/// ```
#[macro_export]
macro_rules! map_err_logged {
    (&$code:expr) => {
        $crate::map_err_logged!($code)
    };
    (&$code:expr, $message:expr) => {
        $crate::map_err_logged!($code, $message)
    };
    ($code:expr) => {{
        move |err| {
			use $crate::result::ErrCodeTrait;
            let code = $code;

            tracing::error!(
                code = code.domain_code(),
                message = code.message(),
                error = ?err,
            );

            $crate::result::AppError::from_code(code)
        }
    }};
    ($code:expr, $message:expr) => {{
        move |err| {
			use $crate::result::ErrCodeTrait;
            let code = $code;

            tracing::error!(
                code = code.domain_code(),
                message = code.message(),
                error = ?err,
            );

            $crate::result::AppError::from_code_msg(code, $message)
        }
    }};
}

/// Builds a lazy closure for [`Option::ok_or_else`] that returns an [`AppError`].
///
/// The generated closure delegates to `app_err!`, so the error is logged only
/// if the option is `None`. The macro accepts either a typed error code or a
/// reference to one. The two-argument form appends additional message context.
///
/// # Examples
///
/// ```rust,ignore
/// let id = maybe_id.ok_or_else(ok_or_logged!(SysErr::InvalidParams, "missing id"))?;
/// ```
#[macro_export]
macro_rules! ok_or_logged {
	(&$code:expr) => {
		$crate::ok_or_logged!($code)
	};
	(&$code:expr, $message:expr) => {
		$crate::ok_or_logged!($code, $message)
	};
	($code:expr) => {{ move || $crate::app_err!($code) }};
	($code:expr, $message:expr) => {{ move || $crate::app_err!($code, $message) }};
}

/// Constructs and logs an [`AppError`] from a typed error code.
///
/// This macro is useful with eager APIs such as [`Option::ok_or`], or when an
/// error value is needed directly. Prefer [`ok_or_logged!`] with
/// [`Option::ok_or_else`] when the message expression is expensive or has side
/// effects.
///
/// The macro accepts either a typed error code or a reference to one. The
/// two-argument form appends additional message context via
/// [`AppError::from_code_msg`].
///
/// # Examples
///
/// ```rust,ignore
/// return Err(app_err!(SysErr::ConfigLoadFailed));
/// ```
#[macro_export]
macro_rules! app_err {
	(&$code:expr) => {
		$crate::app_err!($code)
	};
	(&$code:expr, $message:expr) => {
		$crate::app_err!($code, $message)
	};
	($code:expr) => {{
		use $crate::result::ErrCodeTrait;
		let code = $code;

		tracing::error!(code = code.domain_code(), message = code.message(),);

		$crate::result::AppError::from_code(code)
	}};
	($code:expr, $message:expr) => {{
		use $crate::result::ErrCodeTrait;
		let code = $code;

		tracing::error!(code = code.domain_code(), message = code.message(),);

		$crate::result::AppError::from_code_msg(code, $message)
	}};
}

/// Returns `Err(AppError)` after logging the selected typed error code.
///
/// The one-argument form uses the code's static message. The two-argument form
/// appends extra message context after converting it to [`String`].
///
/// # Examples
///
/// ```rust,ignore
/// err!(SysErr::ConfigLoadFailed)
/// err!(SysErr::InvalidParams, "missing id")
/// ```
#[macro_export]
macro_rules! err {
	($code:expr) => {{
		tracing::error!(err = %$code);
		Err($crate::app_err!($code))
	}};

	($code:expr, $msg:expr) => {{
		tracing::error!(err = %$code, msg = %$msg);
		let msg = ($msg).to_string();
		Err($crate::app_err!($code, msg))
	}};
}

#[cfg(test)]
mod tests {
	use crate::{
		fault::{FaultCode, FaultReport, InfraDetail, InfraFault},
		result::{AppError, SysErr},
	};
	use std::{cell::Cell, rc::Rc};

	#[test]
	fn exposes_structured_code_fields() {
		let err = AppError::from_code(SysErr::ConfigLoadFailed);

		assert_eq!(err.domain(), "SYS");
		assert_eq!(err.code(), 1001);
		assert_eq!(err.message(), "Config load failed");
	}

	#[test]
	fn logged_map_err_maps_source_error_to_app_error() {
		let result: Result<(), &'static str> = Err("raw config error");
		let err = result.map_err(crate::map_err_logged!(SysErr::ConfigLoadFailed));

		assert_eq!(
			err,
			Err(AppError::ErrCode {
				domain: "SYS",
				code: 1001,
				message: "Config load failed".into(),
			})
		);
	}

	#[test]
	fn map_err_appends_dynamic_message_to_static_message() {
		let result: Result<(), &'static str> = Err("raw invalid params");
		let err = result
			.map_err(crate::map_err!(
				&SysErr::InvalidParams,
				format!("invalid rpc header value for key '{}'", "x-request-id")
			))
			.expect_err("source error should be mapped");

		assert_eq!(err.domain(), "SYS");
		assert_eq!(err.code(), 2001);
		assert_eq!(
			err.message(),
			"Invalid params: invalid rpc header value for key 'x-request-id'"
		);
	}

	#[test]
	fn app_error_converts_to_anyhow_error() {
		fn anyhow_result() -> anyhow::Result<()> {
			Err(AppError::from_code(SysErr::ConfigLoadFailed))?
		}

		let err = anyhow_result().expect_err("app error should convert to anyhow error");

		assert_eq!(err.to_string(), "[SYS:1001] Config load failed");
	}

	#[test]
	fn app_error_into_anyhow_keeps_display_message() {
		let err = AppError::from_code_msg(SysErr::InvalidParams, "missing header").into_anyhow();

		assert_eq!(err.to_string(), "[SYS:2001] Invalid params: missing header");
	}

	#[test]
	fn app_error_accepts_compact_fault() {
		let err = AppError::from(InfraFault::ConfigLoad.fault_with(InfraDetail::ConfigPath));

		assert_eq!(err.domain(), "FLT");
		assert_eq!(err.code(), 1002);
		assert!(err.message().contains("domain=1"));
		assert!(err.message().contains("detail=1"));
	}

	#[test]
	fn app_error_accepts_fault_report() {
		let err = AppError::from(FaultReport::from_code_msg(
			InfraFault::InvalidParams,
			InfraDetail::RequestParam,
			"missing id",
		));

		assert_eq!(err.domain(), "FLT");
		assert_eq!(err.code(), 2001);
		assert!(
			err.message()
				.contains("Invalid parameters: detail=request parameter: missing id")
		);
	}

	#[test]
	fn ok_or_maps_none_to_app_error() {
		let result = Option::<()>::None.ok_or_else(crate::ok_or_logged!(&SysErr::InvalidParams));

		assert_eq!(
			result,
			Err(AppError::ErrCode {
				domain: "SYS",
				code: 2001,
				message: "Invalid params".into(),
			})
		);
	}

	#[test]
	fn ok_or_appends_dynamic_message() {
		let result = Option::<()>::None.ok_or_else(crate::ok_or_logged!(
			&SysErr::InvalidParams,
			format!("missing required field '{}'", "name")
		));

		assert_eq!(
			result.expect_err("none option should be mapped").message(),
			"Invalid params: missing required field 'name'"
		);
	}

	#[test]
	fn ok_or_is_lazy_for_some_value() {
		let invoked = Rc::new(Cell::new(false));
		let invoked_in_message = Rc::clone(&invoked);

		let result = Some("value").ok_or_else(crate::ok_or_logged!(SysErr::InvalidParams, {
			invoked_in_message.set(true);
			"should not be evaluated"
		}));

		assert_eq!(result, Ok("value"));
		assert!(!invoked.get());
	}

	#[test]
	fn ok_or_logged_maps_none_to_app_error() {
		let err = Option::<()>::None
			.ok_or_else(crate::ok_or_logged!(SysErr::InvalidParams))
			.expect_err("none option should be mapped");

		assert_eq!(err.domain(), "SYS");
		assert_eq!(err.code(), 2001);
		assert_eq!(err.message(), "Invalid params");
	}

	#[test]
	fn app_err_maps_none_with_ok_or() {
		let result = Option::<()>::None.ok_or(crate::app_err!(&SysErr::InvalidParams));

		assert_eq!(result, Err(AppError::new("SYS", 2001, "Invalid params")),);
	}

	#[test]
	fn app_err_appends_dynamic_message_with_ok_or() {
		let result = Option::<()>::None.ok_or(crate::app_err!(
			SysErr::InvalidParams,
			format!("missing required field '{}'", "name")
		));

		assert_eq!(
			result.expect_err("none option should be mapped").message(),
			"Invalid params: missing required field 'name'"
		);
	}
}

// #[macro_export]
// macro_rules! ok_or {
// 	(&$code:expr) => {
// 		$crate::ok_or!($code)
// 	};
// 	(&$code:expr, $message:expr) => {
// 		$crate::ok_or!($code, $message)
// 	};
// 	($code:expr) => {{
// 		move || {
// 			let code = $code;
// 			let app_error = $crate::result::AppError::from_code(code);
//
// 			tracing::debug!(
// 				domain = app_error.domain(),
// 				err_code = app_error.code(),
// 				message = app_error.message()
// 			);
//
// 			app_error
// 		}
// 	}};
// 	($code:expr, $message:expr) => {{
// 		move || {
// 			let code = $code;
// 			let app_error = $crate::result::AppError::from_code_msg(code, $message);
//
// 			tracing::debug!(
// 				domain = app_error.domain(),
// 				err_code = app_error.code(),
// 				message = app_error.message(),
// 			);
//
// 			app_error
// 		}
// 	}};
// }
