use crate::fault::{Fault, FaultCode, FaultDetail};
use core::fmt;
use std::borrow::Cow;

/// Cold-path fault report with optional human context.
///
/// This type is intentionally separate from [`Fault`]. Construct it only at
/// boundaries where allocation and formatting are acceptable.
#[derive(thiserror::Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct FaultReport {
	fault: Fault,
	message: Cow<'static, str>,
}

impl FaultReport {
	/// Creates a report from a compact fault and message.
	pub fn new(fault: Fault, message: impl Into<Cow<'static, str>>) -> Self {
		Self {
			fault,
			message: message.into(),
		}
	}

	/// Creates a report from a typed code and zero detail.
	pub fn from_code<C>(code: C) -> Self
	where
		C: FaultCode,
	{
		Self::new(code.fault(), Cow::Borrowed(code.message()))
	}

	/// Creates a report from a typed code, typed detail, and extra context.
	pub fn from_code_msg<C, D, M>(code: C, detail: D, message: M) -> Self
	where
		C: FaultCode,
		D: FaultDetail,
		M: Into<String>,
	{
		let code_message = code.message();
		let detail_message = detail.detail();
		let extra = message.into();
		let message = if extra.is_empty() {
			let mut message = String::with_capacity(code_message.len() + 10 + detail_message.len());
			message.push_str(code_message);
			message.push_str(": detail=");
			message.push_str(detail_message);
			Cow::Owned(message)
		} else {
			let mut message = String::with_capacity(code_message.len() + 12 + detail_message.len() + extra.len());
			message.push_str(code_message);
			message.push_str(": detail=");
			message.push_str(detail_message);
			message.push_str(": ");
			message.push_str(&extra);
			Cow::Owned(message)
		};

		Self::new(code.fault_with(detail), message)
	}

	/// Returns the compact fault.
	pub fn fault(&self) -> Fault {
		self.fault
	}

	/// Returns the report message.
	pub fn message(&self) -> &str {
		&self.message
	}

	/// Consumes the report into its compact fault.
	pub fn into_fault(self) -> Fault {
		self.fault
	}

	/// Consumes the report into its compact fault and message.
	pub fn into_parts(self) -> (Fault, Cow<'static, str>) {
		(self.fault, self.message)
	}
}

impl From<Fault> for FaultReport {
	fn from(value: Fault) -> Self {
		Self::new(value, fault_message(value))
	}
}

impl fmt::Debug for FaultReport {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.message)
	}
}

fn fault_message(fault: Fault) -> Cow<'static, str> {
	match (fault.domain(), fault.code_id()) {
		(1, 1) => Cow::Borrowed(crate::fault::InfraFault::Unknown.message()),
		(1, 1001) => Cow::Borrowed(crate::fault::InfraFault::ConfigMissing.message()),
		(1, 1002) => Cow::Borrowed(crate::fault::InfraFault::ConfigLoad.message()),
		(1, 2001) => Cow::Borrowed(crate::fault::InfraFault::InvalidParams.message()),
		(1, 3001) => Cow::Borrowed(crate::fault::InfraFault::RuntimeInit.message()),
		(1, 4001) => Cow::Borrowed(crate::fault::InfraFault::Io.message()),
		_ => Cow::Owned(format!(
			"fault domain={} code={} detail={} raw={}",
			fault.domain(),
			fault.code_id(),
			fault.detail(),
			fault.raw()
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::FaultReport;
	use crate::fault::{FaultCode, InfraDetail, InfraFault};

	#[test]
	fn report_adds_context_only_on_cold_path() {
		let report = FaultReport::from_code_msg(InfraFault::InvalidParams, InfraDetail::RequestParam, "missing id");

		assert_eq!(
			report.fault(),
			InfraFault::InvalidParams.fault_with(InfraDetail::RequestParam)
		);
		assert_eq!(
			report.message(),
			"Invalid parameters: detail=request parameter: missing id"
		);
		assert_eq!(
			report.to_string(),
			"Invalid parameters: detail=request parameter: missing id"
		);
		assert_eq!(
			format!("{report:?}"),
			"Invalid parameters: detail=request parameter: missing id"
		);
	}

	#[test]
	fn report_recovers_infra_message_from_fault() {
		let report = FaultReport::from(InfraFault::ConfigLoad.fault_with(InfraDetail::ConfigPath));

		assert_eq!(report.message(), "Configuration load failed");
		assert_eq!(report.to_string(), "Configuration load failed");
	}
}
