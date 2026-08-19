use crate::{
	errors::{self, RdbDetail},
	typed_db::batch::{SchemaBatch, SchemaBatchRows, WriteOp},
};
use infra_core::result::AppResult;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DurableWriteOp {
	Value { key: Vec<u8>, value: Vec<u8> },
	Deletion { key: Vec<u8> },
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
