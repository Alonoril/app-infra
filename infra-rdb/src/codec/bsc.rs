/// A macro to generate the `KeyCodec` and `ValueCodec` implementations for a given schema type with BCS.
#[macro_export]
macro_rules! impl_schema_bcs_codec {
	($schema_type:ty, $key_type:ty, $value_type:ty) => {
		impl $crate::typed_db::schema::KeyCodec<$schema_type> for $key_type {
			fn encode_key(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bcs::to_bytes(self).map_err($crate::errors::from_bcs_error)
			}

			fn decode_key(data: &[u8]) -> infra_core::result::AppResult<Self> {
				bcs::from_bytes(data).map_err($crate::errors::from_bcs_error)
			}
		}

		impl $crate::typed_db::schema::ValueCodec<$schema_type> for $value_type {
			fn encode_value(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bcs::to_bytes(self).map_err($crate::errors::from_bcs_error)
			}

			fn decode_value(data: &[u8]) -> infra_core::result::AppResult<Self> {
				bcs::from_bytes(data).map_err($crate::errors::from_bcs_error)
			}
		}
	};
}

/// A macro to generate the `KeyCodec` implementation for a given schema type.
#[macro_export]
macro_rules! impl_schema_key_bcs_codec {
	($schema_type:ty, $key_type:ty) => {
		impl $crate::typed_db::schema::KeyCodec<$schema_type> for $key_type {
			fn encode_key(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bcs::to_bytes(self).map_err($crate::errors::from_bcs_error)
			}

			fn decode_key(data: &[u8]) -> infra_core::result::AppResult<Self> {
				bcs::from_bytes(data).map_err($crate::errors::from_bcs_error)
			}
		}
	};
}

/// A macro to generate the `ValueCodec` implementation for a given schema type.
#[macro_export]
macro_rules! impl_schema_value_bcs_codec {
	($schema_type:ty, $value_type:ty) => {
		impl $crate::typed_db::schema::ValueCodec<$schema_type> for $value_type {
			fn encode_value(&self) -> infra_core::result::AppResult<Vec<u8>> {
				bcs::to_bytes(self).map_err($crate::errors::from_bcs_error)
			}

			fn decode_value(data: &[u8]) -> infra_core::result::AppResult<Self> {
				bcs::from_bytes(data).map_err($crate::errors::from_bcs_error)
			}
		}
	};
}
