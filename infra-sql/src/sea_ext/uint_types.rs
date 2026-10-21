use bigdecimal::BigDecimal;
use ruint::aliases::{U128, U256};
use sea_orm::{
	ColIdx, DbErr, QueryResult, TryGetError, TryGetable,
	sea_query::{ArrayType, ColumnType, Nullable, Value, ValueType, ValueTypeErr},
};
use serde::{Deserialize, Serialize};
use std::{
	fmt::{self, Display},
	str::FromStr,
};

// Macro: generate basic DbUxxx wrapper types
macro_rules! define_db_uint_wrapper {
	// Simple types (e.g., u64), direct wrapper
	($wrapper_name:ident, $inner_type:ty) => {
		#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
		pub struct $wrapper_name(pub $inner_type);

		impl Display for $wrapper_name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				write!(f, "{}", self.0)
			}
		}

		impl From<$inner_type> for $wrapper_name {
			fn from(v: $inner_type) -> Self {
				$wrapper_name(v)
			}
		}

		impl From<$wrapper_name> for $inner_type {
			fn from(v: $wrapper_name) -> Self {
				v.0
			}
		}
	};
	// Complex types (e.g., U128, U256), require custom serialization
	($wrapper_name:ident, $inner_type:ty, with_custom_serde) => {
		#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
		pub struct $wrapper_name(pub $inner_type);

		impl Display for $wrapper_name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				write!(f, "{}", self.0)
			}
		}

		impl From<$inner_type> for $wrapper_name {
			fn from(v: $inner_type) -> Self {
				$wrapper_name(v)
			}
		}

		impl From<$wrapper_name> for $inner_type {
			fn from(v: $wrapper_name) -> Self {
				v.0
			}
		}
	};
}

// Macro: implement Serialize/Deserialize for types requiring custom serialization
macro_rules! impl_db_uint_serde {
	($wrapper_name:ident, $inner_type:ty) => {
		impl Serialize for $wrapper_name {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: serde::Serializer,
			{
				self.0.to_string().serialize(serializer)
			}
		}

		impl<'de> Deserialize<'de> for $wrapper_name {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: serde::Deserializer<'de>,
			{
				let s = String::deserialize(deserializer)?;
				<$inner_type>::from_str(&s)
					.map($wrapper_name)
					.map_err(serde::de::Error::custom)
			}
		}
	};
}

// Macro: implement TryGetable trait
macro_rules! impl_db_uint_try_getable {
	// Special handling for u64: convert from i64
	($wrapper_name:ident, u64) => {
		impl TryGetable for $wrapper_name {
			fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
				// PostgreSQL BIGINT can store up to 9,223,372,036,854,775,807 (i64::MAX)
				// For u64, we need to handle values > i64::MAX
				let val = i64::try_get_by(res, idx)?;
				if val < 0 {
					Err(TryGetError::Null(format!("{:?}", idx)))
				} else {
					Ok($wrapper_name(val as u64))
				}
			}
		}
	};
	// U128/U256 etc.: convert from BigDecimal
	($wrapper_name:ident, $inner_type:ty) => {
		impl TryGetable for $wrapper_name {
			fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
				// Get as BigDecimal from PostgreSQL
				let big_decimal = BigDecimal::try_get_by(res, idx)?;
				// Use our TryFrom implementation for better error handling
				big_decimal
					.try_into()
					.map_err(|e: &'static str| TryGetError::DbErr(DbErr::Type(e.to_string())))
			}
		}
	};
}

// Macro: implement ValueType-related traits
macro_rules! impl_db_uint_value_type {
	// u64 version
	($wrapper_name:ident, u64) => {
		impl ValueType for $wrapper_name {
			fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
				match v {
					Value::BigInt(Some(x)) => {
						if x < 0 {
							Err(ValueTypeErr)
						} else {
							Ok($wrapper_name(x as u64))
						}
					}
					Value::BigUnsigned(Some(x)) => Ok($wrapper_name(x)),
					_ => Err(ValueTypeErr),
				}
			}

			fn type_name() -> String {
				stringify!($wrapper_name).to_owned()
			}

			fn array_type() -> ArrayType {
				ArrayType::BigInt
			}

			fn column_type() -> ColumnType {
				ColumnType::BigInteger
			}
		}

		impl From<$wrapper_name> for Value {
			fn from(v: $wrapper_name) -> Self {
				Value::BigUnsigned(Some(v.0))
			}
		}

		impl Nullable for $wrapper_name {
			fn null() -> Value {
				Value::BigUnsigned(None)
			}
		}
	};
	// BigDecimal version (U128/U256)
	($wrapper_name:ident, $inner_type:ty, $precision:expr) => {
		impl ValueType for $wrapper_name {
			fn try_from(v: Value) -> Result<Self, ValueTypeErr> {
				match v {
					Value::BigDecimal(Some(x)) => {
						let str_val = x.to_string();
						<$inner_type>::from_str_radix(&str_val, 10)
							.map($wrapper_name)
							.map_err(|_| ValueTypeErr)
					}
					_ => Err(ValueTypeErr),
				}
			}

			fn type_name() -> String {
				stringify!($wrapper_name).to_owned()
			}

			fn array_type() -> ArrayType {
				ArrayType::BigDecimal
			}

			fn column_type() -> ColumnType {
				ColumnType::Decimal(Some(($precision, 0)))
			}
		}

		impl From<$wrapper_name> for Value {
			fn from(v: $wrapper_name) -> Self {
				// BigDecimal can handle arbitrary precision
				let str_val = v.0.to_string();
				match BigDecimal::from_str(&str_val) {
					Ok(big_decimal) => Value::BigDecimal(Some(Box::new(big_decimal))),
					Err(_) => {
						// This should never happen for valid values
						panic!(
							"Failed to convert {} to BigDecimal: {}",
							stringify!($inner_type),
							str_val
						)
					}
				}
			}
		}

		impl Nullable for $wrapper_name {
			fn null() -> Value {
				Value::BigDecimal(None)
			}
		}
	};
}

// Generate DbU64 via macro
define_db_uint_wrapper!(DbU64, u64);
impl_db_uint_try_getable!(DbU64, u64);

// Generate DbU128 via macro
define_db_uint_wrapper!(DbU128, U128, with_custom_serde);
impl_db_uint_serde!(DbU128, U128);
impl_db_uint_try_getable!(DbU128, U128);

// Generate DbU256 via macro
define_db_uint_wrapper!(DbU256, U256, with_custom_serde);
impl_db_uint_serde!(DbU256, U256);
impl_db_uint_try_getable!(DbU256, U256);

// Implement ValueType-related traits via macros
impl_db_uint_value_type!(DbU64, u64);

// Implement ValueType-related traits via macros
// U128 max is ~340 undecillion (38 digits), so NUMERIC(39,0) is sufficient
impl_db_uint_value_type!(DbU128, U128, 39);

// Implement ValueType-related traits via macros
// U256 max is ~115 quattuorvigintillion (78 digits), so NUMERIC(78,0) is sufficient
impl_db_uint_value_type!(DbU256, U256, 78);
