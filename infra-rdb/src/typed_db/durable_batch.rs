use crate::{
	errors::{self, RdbDetail},
	typed_db::batch::{SchemaBatch, SchemaBatchRows, WriteOp},
};
use infra_core::result::AppResult;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DurableWriteOp {
	Value {
		#[serde(serialize_with = "serialize_buffer")]
		key: Vec<u8>,
		#[serde(serialize_with = "serialize_buffer")]
		value: Vec<u8>,
	},
	Deletion {
		#[serde(serialize_with = "serialize_buffer")]
		key: Vec<u8>,
	},
}

// Binary serializers encode bytes as the same length-prefixed u8 sequence, but can copy the
// whole buffer instead of visiting each byte. Deserialization retains the legacy Vec format.
fn serialize_buffer<S: serde::Serializer>(buffer: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
	serializer.serialize_bytes(buffer)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	#[derive(Serialize)]
	struct Buffer<'a>(#[serde(serialize_with = "serialize_buffer")] &'a [u8]);

	#[test]
	fn bulk_buffer_serialization_keeps_legacy_wire_format() {
		let buffer = [0, 127, 128, 255];
		let encoded = bcs::to_bytes(&Buffer(&buffer)).unwrap();
		assert_eq!(encoded, vec![4, 0, 127, 128, 255]);
		let legacy: Vec<u8> = bcs::from_bytes(&encoded).unwrap();
		assert_eq!(legacy, buffer);
	}

	#[test]
	fn durable_operations_decode_legacy_key_and_value_buffers() {
		let encoded = vec![0, 2, 41, 42, 3, 0, 128, 255];
		let operation: DurableWriteOp = bcs::from_bytes(&encoded).unwrap();
		assert_eq!(
			operation,
			DurableWriteOp::Value {
				key: vec![41, 42],
				value: vec![0, 128, 255]
			}
		);
		assert_eq!(bcs::to_bytes(&operation).unwrap(), encoded);
	}

	#[test]
	fn append_schema_batch_preserves_operation_order_and_existing_buffers() {
		let original_value = vec![7; 32];
		let original_pointer = original_value.as_ptr();
		let mut batch = DurableWriteBatch {
			column_families: vec![DurableColumnFamilyBatch {
				column_family: "facts".to_owned(),
				operations: vec![DurableWriteOp::Value {
					key: vec![1],
					value: original_value,
				}],
			}],
		};
		let tail = SchemaBatch::from_rows(HashMap::from([
			(Cow::Borrowed("facts"), vec![WriteOp::Deletion { key: vec![1] }]),
			(
				Cow::Borrowed("meta"),
				vec![WriteOp::Value {
					key: vec![2],
					value: vec![3],
				}],
			),
		]));

		batch.append_schema_batch(tail);

		assert_eq!(batch.column_families.len(), 2);
		let facts = &batch.column_families[0].operations;
		assert!(matches!(&facts[0], DurableWriteOp::Value { value, .. } if value.as_ptr() == original_pointer));
		assert!(matches!(&facts[1], DurableWriteOp::Deletion { key } if key == &[1]));
		assert!(
			batch
				.column_families
				.iter()
				.any(|column_family| column_family.column_family == "meta")
		);
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableColumnFamilyBatch {
	pub column_family: String,
	pub operations: Vec<DurableWriteOp>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableWriteBatch {
	pub column_families: Vec<DurableColumnFamilyBatch>,
}

impl DurableWriteBatch {
	/// Appends typed operations after the existing operations in each column family.
	/// Existing key and value buffers stay owned by this batch without conversion.
	pub fn append_schema_batch(&mut self, batch: SchemaBatch) {
		for mut incoming in Self::from_schema_batch(batch).column_families {
			if let Some(existing) = self
				.column_families
				.iter_mut()
				.find(|existing| existing.column_family == incoming.column_family)
			{
				existing.operations.append(&mut incoming.operations);
			} else {
				self.column_families.push(incoming);
			}
		}
	}

	pub fn from_schema_batch(batch: SchemaBatch) -> Self {
		let column_families = batch
			.into_rows()
			.into_iter()
			.map(|(column_family, operations)| DurableColumnFamilyBatch {
				column_family: column_family.into_owned(),
				operations: operations
					.into_iter()
					.map(|operation| match operation {
						WriteOp::Value { key, value } => DurableWriteOp::Value { key, value },
						WriteOp::Deletion { key } => DurableWriteOp::Deletion { key },
					})
					.collect(),
			})
			.collect();

		Self { column_families }
	}

	pub fn into_schema_batch(self) -> AppResult<SchemaBatch> {
		let mut rows = SchemaBatchRows::with_capacity(self.column_families.len());
		for DurableColumnFamilyBatch {
			column_family,
			operations,
		} in self.column_families
		{
			let entry = rows.entry(Cow::Owned(column_family));
			let std::collections::hash_map::Entry::Vacant(entry) = entry else {
				return Err(errors::invalid_params(RdbDetail::ColumnFamily));
			};
			entry.insert(
				operations
					.into_iter()
					.map(|operation| match operation {
						DurableWriteOp::Value { key, value } => WriteOp::Value { key, value },
						DurableWriteOp::Deletion { key } => WriteOp::Deletion { key },
					})
					.collect(),
			);
		}

		Ok(SchemaBatch::from_rows(rows))
	}
}
