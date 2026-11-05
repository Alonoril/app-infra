use crate::sea_ext::uint_types::{DbU128, DbU256};
use bigdecimal::{
	BigDecimal,
	num_bigint::{BigInt, BigUint, Sign},
};
use ruint::aliases::{U128, U256};

/// Macro: implement conversion DbUxxx → BigDecimal
macro_rules! impl_from_dbuint_to_bigdecimal {
	($db_type:ty, $uint_type:ty, $byte_size:expr) => {
		impl From<$db_type> for BigDecimal {
			fn from(value: $db_type) -> Self {
				let value: $uint_type = value.into();
				let buf: [u8; $byte_size] = value.to_be_bytes();
				let big_uint = BigUint::from_bytes_be(&buf);
				let big_int = BigInt::from_biguint(Sign::Plus, big_uint);
				BigDecimal::from(big_int)
			}
		}
	};
}

/// Macro: implement conversion BigDecimal → DbUxxx
macro_rules! impl_try_from_bigdecimal_to_dbuint {
	($db_type:ty, $uint_type:ty, $byte_size:expr) => {
		impl TryFrom<BigDecimal> for $db_type {
			type Error = &'static str;

			fn try_from(value: BigDecimal) -> Result<Self, Self::Error> {
				let (mut big_int, scale) = value.normalized().into_bigint_and_exponent();
				if scale > 0 {
					return Err("BigDecimal has fractional part");
				}
				if big_int.sign() == Sign::Minus {
					return Err("negative value");
				}

				// PostgreSQL binary NUMERIC may decode integers with a negative scale.
				// Bound the exponent before allocating a power of ten.
				if scale < 0 {
					let exponent = scale.unsigned_abs();
					if exponent > ($byte_size * 8) as u64 {
						return Err(concat!("value too large for ", stringify!($uint_type)));
					}
					big_int *= BigInt::from(10).pow(exponent as u32);
				}

				let big_uint = big_int.to_biguint().ok_or("negative value")?;
				let bytes = big_uint.to_bytes_be();
				if bytes.len() > $byte_size {
					return Err(concat!("value too large for ", stringify!($uint_type)));
				}

				// Pad with leading zeros to the target byte length
				let mut buf = [0u8; $byte_size];
				buf[$byte_size - bytes.len()..].copy_from_slice(&bytes);
				Ok(<$uint_type>::from_be_bytes(buf).into())
			}
		}
	};
}

// Use macro to generate implementations
impl_from_dbuint_to_bigdecimal!(DbU256, U256, 32);
impl_from_dbuint_to_bigdecimal!(DbU128, U128, 16);

impl_try_from_bigdecimal_to_dbuint!(DbU256, U256, 32);
impl_try_from_bigdecimal_to_dbuint!(DbU128, U128, 16);

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn integer_scales_preserve_value_and_reject_fractions_and_overflow() {
		for encoded in ["100000000000000000000", "1e20", "100000000000000000000.000", "0.000"] {
			let decimal: BigDecimal = encoded.parse().unwrap();
			let actual = DbU256::try_from(decimal.clone()).unwrap();
			assert_eq!(BigDecimal::from(actual), decimal);
			assert_eq!(BigDecimal::from(DbU128::try_from(decimal.clone()).unwrap()), decimal);
		}
		for encoded in ["0.1", "1.01", "-1", "1e1000"] {
			let decimal: BigDecimal = encoded.parse().unwrap();
			assert!(DbU256::try_from(decimal.clone()).is_err());
			assert!(DbU128::try_from(decimal).is_err());
		}
		let maximum = BigDecimal::from(DbU256::from(U256::MAX));
		assert!(DbU256::try_from(maximum.clone()).is_ok());
		assert!(DbU256::try_from(maximum + BigDecimal::from(1)).is_err());
	}
}
