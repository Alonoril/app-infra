use crate::fault::{Fault, FaultDetail};
use core::fmt::{Debug, Display};

/// Stable machine identity for a typed fault code.
#[derive(Debug, Copy, Clone)]
pub struct FaultMeta {
	/// Stable numeric domain. Keep values stable once published.
	pub domain: u16,
	/// Domain-local numeric code. Keep values stable once published.
	pub code: u16,
}

impl PartialEq for FaultMeta {
	fn eq(&self, other: &Self) -> bool {
		self.domain == other.domain && self.code == other.code
	}
}

impl Eq for FaultMeta {}

/// Trait implemented by small typed fault-code enums.
pub trait FaultCode: Debug + Display + Copy + Sync + Send + 'static {
	/// Returns the stable machine identity for this fault code.
	fn meta(self) -> FaultMeta;

	/// Stable numeric domain.
	#[inline]
	fn domain(self) -> u16 {
		self.meta().domain
	}

	/// Domain-local numeric code.
	#[inline]
	fn code(self) -> u16 {
		self.meta().code
	}

	/// Static machine-oriented name.
	fn name(self) -> &'static str;

	/// Static human-readable message.
	fn message(self) -> &'static str;

	/// Builds a compact [`Fault`] with zero detail.
	#[inline(always)]
	fn fault(self) -> Fault {
		Fault::code(self)
	}

	/// Builds a compact [`Fault`] with typed detail.
	#[inline(always)]
	fn fault_with<D>(self, detail: D) -> Fault
	where
		D: FaultDetail,
	{
		Fault::from_code(self, detail)
	}

	/// Builds a compact [`Fault`] with a raw detail id.
	///
	/// Prefer [`FaultCode::fault_with`] with a typed detail enum unless the id
	/// comes from deserialization, FFI, migration, or a bottom-level system code.
	#[inline(always)]
	fn fault_with_raw(self, detail: u32) -> Fault {
		Fault::from_code_raw(self, detail)
	}
}

/// Declares a small `Copy` enum of stable fault codes.
///
/// The macro keeps names and messages static while the hot-path value remains
/// an 8-byte [`Fault`].
#[macro_export]
macro_rules! define_fault_codes {
	(
		$(
			$(#[$enum_attr:meta])*
			$vis:vis enum $enum_name:ident($domain:literal) {
				$(
					$(#[$variant_attr:meta])*
					$variant_name:ident = ($code:literal, $name:literal, $message:literal),
				)*
			}
		)*
	) => {
		$(
			$(#[$enum_attr])*
			#[repr(u16)]
			#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
			$vis enum $enum_name {
				$(
					$(#[$variant_attr])*
					$variant_name = $code,
				)*
			}

			impl $crate::fault::FaultCode for $enum_name {
				#[inline]
				fn meta(self) -> $crate::fault::FaultMeta {
					match self {
						$(
							Self::$variant_name => $crate::fault::FaultMeta {
								domain: $domain,
								code: $code,
							},
						)*
					}
				}

				#[inline]
				fn domain(self) -> u16 {
					$domain
				}

				#[inline]
				fn code(self) -> u16 {
					self as u16
				}

				#[inline]
				fn name(self) -> &'static str {
					match self {
						$(
							Self::$variant_name => $name,
						)*
					}
				}

				#[inline]
				fn message(self) -> &'static str {
					match self {
						$(
							Self::$variant_name => $message,
						)*
					}
				}
			}

			impl std::fmt::Display for $enum_name {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					use $crate::fault::FaultCode;

					write!(f, "[{}:{}] {}", self.domain(), self.code(), self.name())
				}
			}

			impl From<$enum_name> for $crate::fault::Fault {
				#[inline(always)]
				fn from(value: $enum_name) -> Self {
					$crate::fault::Fault::code(value)
				}
			}
		)*
	};
}

define_fault_codes! {
	/// Built-in infrastructure fault codes.
	pub enum InfraFault(1) {
		/// Unknown infrastructure fault.
		Unknown = (1, "unknown", "Unknown infrastructure fault"),
		/// A required configuration value or file is missing.
		ConfigMissing = (1001, "config_missing", "Configuration is missing"),
		/// Configuration exists but cannot be loaded or decoded.
		ConfigLoad = (1002, "config_load", "Configuration load failed"),
		/// A caller supplied invalid parameters.
		InvalidParams = (2001, "invalid_params", "Invalid parameters"),
		/// Runtime setup failed.
		RuntimeInit = (3001, "runtime_init", "Runtime initialization failed"),
		/// IO failed at an infrastructure boundary.
		Io = (4001, "io", "IO operation failed"),
	}
}

#[cfg(test)]
mod tests {
	use super::{FaultCode, InfraFault};
	use crate::fault::{Fault, InfraDetail};

	#[test]
	fn infra_fault_builds_fixed_fault() {
		let fault = InfraFault::ConfigLoad.fault_with(InfraDetail::ConfigPath);

		assert_eq!(fault, Fault::new(1, 1002, 1));
	}

	#[test]
	fn infra_fault_can_still_build_from_raw_detail() {
		let fault = InfraFault::ConfigLoad.fault_with_raw(9);

		assert_eq!(fault, Fault::new(1, 1002, 9));
	}

	#[test]
	fn meta_equality_ignores_text() {
		assert_eq!(InfraFault::InvalidParams.meta(), InfraFault::InvalidParams.meta());
	}

	#[test]
	fn text_is_exposed_by_trait_methods() {
		assert_eq!(InfraFault::InvalidParams.name(), "invalid_params");
		assert_eq!(InfraFault::InvalidParams.message(), "Invalid parameters");
	}
}
