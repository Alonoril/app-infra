use infra_core::result::{AppError, ErrCodeTrait, SysErr};
use serde::{Serialize, Serializer};
use std::{
	borrow::Cow,
	fmt::{self, Write},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespCode {
	Static(&'static str),
	Parts { domain: &'static str, code: u16 },
}

impl RespCode {
	#[inline]
	pub const fn from_static(code: &'static str) -> Self {
		Self::Static(code)
	}
}

impl fmt::Display for RespCode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Static(code) => f.write_str(code),
			Self::Parts { domain, code } => write!(f, "{domain}:{code}"),
		}
	}
}

impl Serialize for RespCode {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match self {
			Self::Static(code) => serializer.serialize_str(code),
			Self::Parts { .. } => serializer.collect_str(self),
		}
	}
}

#[derive(Debug, Clone, Serialize)]
pub struct RespData<T = ()> {
	pub code: RespCode,
	pub msg: Cow<'static, str>,
	pub data: Option<T>,
}

impl<T> RespData<T> {
	pub fn success(data: T) -> Self {
		Self {
			code: RespCode::from_static(SysErr::Success.domain_code()),
			msg: Cow::Borrowed(SysErr::Success.message()),
			data: Some(data),
		}
	}
}

impl RespData<()> {
	pub fn with_code<C>(code: C) -> Self
	where
		C: ErrCodeTrait,
	{
		Self {
			code: RespCode::from_static(code.domain_code()),
			msg: Cow::Borrowed(code.message()),
			data: None,
		}
	}

	pub fn with_ext_msg<C>(code: C, ext_msg: impl AsRef<str>) -> RespData
	where
		C: ErrCodeTrait,
	{
		let code_msg = code.message();
		let ext_msg = ext_msg.as_ref();
		let mut msg = String::with_capacity(code_msg.len() + 1 + ext_msg.len());
		msg.push_str(code_msg);
		msg.push(' ');
		msg.push_str(ext_msg);

		Self {
			code: RespCode::from_static(code.domain_code()),
			msg: Cow::Owned(msg),
			data: None,
		}
	}

	pub fn with_display_msg<C, M>(code: C, msg: M) -> RespData
	where
		C: ErrCodeTrait,
		M: fmt::Display,
	{
		let code_msg = code.message();
		let msg = if code_msg.is_empty() {
			Cow::Owned(msg.to_string())
		} else {
			let mut merged_msg = String::with_capacity(code_msg.len() + 2);
			merged_msg.push_str(code_msg);
			merged_msg.push_str(": ");
			write!(&mut merged_msg, "{msg}").expect("writing to String cannot fail");
			Cow::Owned(merged_msg)
		};

		Self {
			code: RespCode::from_static(code.domain_code()),
			msg,
			data: None,
		}
	}

	pub fn with_anyhow<C>(code: C, err: anyhow::Error) -> RespData
	where
		C: ErrCodeTrait,
	{
		let code_msg = code.message();
		let err_msg = err.to_string();
		let msg = if code_msg.is_empty() {
			Cow::Owned(err_msg)
		} else {
			let mut msg = String::with_capacity(code_msg.len() + 2 + err_msg.len());
			msg.push_str(code_msg);
			msg.push_str(": ");
			msg.push_str(&err_msg);
			Cow::Owned(msg)
		};

		Self {
			code: RespCode::from_static(code.domain_code()),
			msg,
			data: None,
		}
	}

	pub fn with_app_error(error: AppError) -> RespData {
		let AppError::ErrCode { domain, code, message } = error;

		RespData {
			code: RespCode::Parts { domain, code },
			msg: message,
			data: None,
		}
	}
}

impl RespData {
	pub fn with(code: &'static str, msg: &'static str) -> Self {
		Self {
			code: RespCode::from_static(code),
			msg: Cow::Borrowed(msg),
			data: None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::RespData;

	#[test]
	fn success_resp_code_is_zero() {
		let resp = RespData::success(());

		assert_eq!(resp.code.to_string(), "000000");
	}
}
