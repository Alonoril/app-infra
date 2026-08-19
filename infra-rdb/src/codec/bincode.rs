/// ```rust
/// use infra_core::result::AppResult;
///
/// pub struct SchemaKey;
///
/// impl SchemaKey {
/// 	pub fn encode(&self) -> AppResult<Vec<u8>> {
/// 		Ok(vec![])
/// 	}
///
/// 	pub fn decode(bytes: &[u8]) -> AppResult<SchemaKey> {
/// 		Ok(SchemaKey)
/// 	}
/// }
/// ```
#[macro_export]
macro_rules! impl_schema_key_codec {
	($schema_type:ty, $key_type:ty) => {
		impl $crate::typed_db::schema::KeyCodec<$schema_type> for $key_type {
			fn encode_key(&self) -> infra_core::result::AppResult<Vec<u8>> {
				self.encode()
			}

			fn decode_key(data: &[u8]) -> infra_core::result::AppResult<Self> {
				Self::decode(data)
			}
		}
	};
}

/// ```rust
/// use infra_core::result::AppResult;
///
/// pub struct SchemaValue;
///
/// impl SchemaValue {
/// 	pub fn encode(&self) -> AppResult<Vec<u8>> {
/// 		Ok(vec![])
/// 	}
///
/// 	pub fn decode(bytes: &[u8]) -> AppResult<SchemaValue> {
/// 		Ok(SchemaValue)
/// 	}
/// }
/// ```
#[macro_export]
macro_rules! impl_schema_value_codec {
	($schema_type:ty, $value_type:ty) => {
		impl $crate::typed_db::schema::ValueCodec<$schema_type> for $value_type {
			fn encode_value(&self) -> infra_core::result::AppResult<Vec<u8>> {
				self.encode()
			}

			fn decode_value(data: &[u8]) -> infra_core::result::AppResult<Self> {
				Self::decode(data)
			}
		}
	};
}

/// A macro to generate the `KeyCodec` and `ValueCodec` implementations for a given schema type  with bincode.
#[macro_export]
macro_rules! impl_schema_bin_codec {
	($schema_type:ty, $key_type:ty, $value_type:ty) => {
		impl $crate::typed_db::schema::KeyCodec<$schema_type> for $key_type {
			fn encode_key(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bincode::encode_to_vec(self, bincode::config::standard())
					.map_err($crate::errors::from_bincode_encode_error)
			}

			fn decode_key(data: &[u8]) -> infra_core::result::AppResult<Self> {
				let (value, _) = bincode::decode_from_slice(data, bincode::config::standard())
					.map_err($crate::errors::from_bincode_decode_error)?;
				Ok(value)
			}
		}

		impl $crate::typed_db::schema::ValueCodec<$schema_type> for $value_type {
			fn encode_value(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bincode::encode_to_vec(self, bincode::config::standard())
					.map_err($crate::errors::from_bincode_encode_error)
			}

			fn decode_value(data: &[u8]) -> infra_core::result::AppResult<Self> {
				let (value, _) = bincode::decode_from_slice(data, bincode::config::standard())
					.map_err($crate::errors::from_bincode_decode_error)?;
				Ok(value)
			}
		}
	};
}

/// A macro to generate the `ValueCodec` implementations for a given schema type  with bincode.
#[macro_export]
macro_rules! impl_schema_value_bin_codec {
	($schema_type:ty, $value_type:ty) => {
		impl $crate::typed_db::schema::ValueCodec<$schema_type> for $value_type {
			fn encode_value(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bincode::encode_to_vec(self, bincode::config::standard())
					.map_err($crate::errors::from_bincode_encode_error)
			}

			fn decode_value(data: &[u8]) -> infra_core::result::AppResult<Self> {
				let (value, _) = bincode::decode_from_slice(data, bincode::config::standard())
					.map_err($crate::errors::from_bincode_decode_error)?;
				Ok(value)
			}
		}
	};
}
