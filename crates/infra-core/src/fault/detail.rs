/// Typed fault detail encoded as a compact `u32`.
///
/// Detail enums give names to otherwise opaque detail ids without changing the
/// fixed 8-byte [`Fault`](crate::fault::Fault) representation.
pub trait FaultDetail: Copy + Send + Sync + 'static {
	/// Returns the compact id stored in [`Fault`](crate::fault::Fault).
	fn id(self) -> u32;

	/// Returns the static human-readable meaning of this detail id.
	fn detail(self) -> &'static str;

	/// Rebuilds a typed detail from a compact id.
	fn from_id(id: u32) -> Self;
}

/// Raw fallback detail used when no domain-specific detail enum is selected.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawFaultDetail(u32);

impl RawFaultDetail {
	/// Creates a raw detail id.
	#[inline(always)]
	pub const fn new(id: u32) -> Self {
		Self(id)
	}
}

impl FaultDetail for RawFaultDetail {
	#[inline(always)]
	fn id(self) -> u32 {
		self.0
	}

	#[inline(always)]
	fn detail(self) -> &'static str {
		"raw detail id"
	}

	#[inline(always)]
	fn from_id(id: u32) -> Self {
		Self(id)
	}
}

/// Declares a compact detail enum with static descriptions.
///
/// Unknown ids are preserved in the fallback variant, so decoding never loses
/// the original `u32` detail carried by [`Fault`](crate::fault::Fault).
///
/// Generated enums use `#[repr(u32)]` so known variants have stable
/// discriminants matching their stored ids. The enum still carries a `Raw(u32)`
/// fallback, so it is a typed boundary/reporting view rather than the hot-path
/// packed representation. Keep [`Fault`](crate::fault::Fault) or
/// [`RawFaultDetail`] on hot paths and decode with
/// [`FaultDetail::from_id`] only when rendering logs, reports, or boundary
/// messages.
#[macro_export]
macro_rules! define_fault_details {
	(
		$(
			$(#[$enum_attr:meta])*
			$vis:vis enum $enum_name:ident {
				$(
					$(#[$variant_attr:meta])*
					$variant_name:ident = ($id:literal, $detail:literal),
				)*
				;
				$(#[$raw_attr:meta])*
				$raw_variant:ident = (_, $raw_detail:literal),
			}
		)*
	) => {
		$(
			$(#[$enum_attr])*
			#[repr(u32)]
			#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
			$vis enum $enum_name {
				$(
					$(#[$variant_attr])*
					$variant_name = $id,
				)*
				$(#[$raw_attr])*
				$raw_variant(u32),
			}

			impl $crate::fault::FaultDetail for $enum_name {
				#[inline]
				fn id(self) -> u32 {
					match self {
						$(
							Self::$variant_name => $id,
						)*
						Self::$raw_variant(id) => id,
					}
				}

				#[inline]
				fn detail(self) -> &'static str {
					match self {
						$(
							Self::$variant_name => $detail,
						)*
						Self::$raw_variant(_) => $raw_detail,
					}
				}

				#[inline]
				fn from_id(id: u32) -> Self {
					match id {
						$(
							$id => Self::$variant_name,
						)*
						id => Self::$raw_variant(id),
					}
				}
			}
		)*
	};
}

define_fault_details! {
	/// Built-in infrastructure detail ids.
	pub enum InfraDetail {
		/// No extra detail.
		None = (0, "no detail"),
		/// Configuration path related detail.
		ConfigPath = (1, "config file path"),
		/// Configuration format related detail.
		ConfigFormat = (2, "config file format"),
		/// Request or function parameter detail.
		RequestParam = (3, "request parameter"),
		/// Runtime setup detail.
		Runtime = (4, "runtime setup"),
		/// IO or OS error code detail.
		IoCode = (5, "io or os error code"),
		;
		/// Unknown raw detail id.
		Raw = (_, "raw detail id"),
	}
}

#[cfg(test)]
mod tests {
	use super::{FaultDetail, InfraDetail, RawFaultDetail};
	use core::mem::size_of;

	#[test]
	fn infra_detail_exposes_static_meaning() {
		assert_eq!(InfraDetail::ConfigPath.id(), 1);
		assert_eq!(InfraDetail::ConfigPath.detail(), "config file path");
	}

	#[test]
	fn infra_detail_preserves_unknown_id() {
		let detail = InfraDetail::from_id(99);

		assert_eq!(detail, InfraDetail::Raw(99));
		assert_eq!(detail.id(), 99);
		assert_eq!(detail.detail(), "raw detail id");
	}

	#[test]
	fn raw_fault_detail_round_trips_id() {
		let detail = RawFaultDetail::from_id(42);

		assert_eq!(detail.id(), 42);
		assert_eq!(detail.detail(), "raw detail id");
	}

	#[test]
	fn infra_detail_is_boundary_view_with_raw_fallback() {
		assert_eq!(size_of::<RawFaultDetail>(), 4);
		assert_eq!(size_of::<InfraDetail>(), 8);
	}
}
