use crate::typed_db::schema::{KeyCodec, Schema, ValueCodec};
use infra_core::result::AppResult;
use std::{borrow::Cow, collections::HashMap, sync::Mutex};

pub type ColumnFamilyName = &'static str;

#[derive(Debug)]
pub enum WriteOp {
	Value { key: Vec<u8>, value: Vec<u8> },
	Deletion { key: Vec<u8> },
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn append_preserves_repeated_key_operation_order_and_moves_buffers() {
		let key = vec![41];
		let value = vec![9; 32];
		let pointer = value.as_ptr();
		let mut first = SchemaBatch::from_rows(HashMap::from([(
			Cow::Borrowed("facts"),
			vec![WriteOp::Deletion { key: key.clone() }],
		)]));
		let second = SchemaBatch::from_rows(HashMap::from([(
			Cow::Borrowed("facts"),
			vec![WriteOp::Value { key, value }],
		)]));
		first.append(second);
		let mut rows = first.into_rows();
		let operations = rows.remove("facts").unwrap();
		assert!(matches!(&operations[0], WriteOp::Deletion { key } if key == &[41]));
		assert!(
			matches!(&operations[1], WriteOp::Value { key, value } if key == &[41] && value == &[9; 32] && value.as_ptr() == pointer)
		);
	}
}

pub(crate) type SchemaBatchRows = HashMap<Cow<'static, str>, Vec<WriteOp>>;

/// `SchemaBatch` holds a consolidate of updates that can be applied to a DB atomically. The updates
/// will be applied in the order in which they are added to the `SchemaBatch`.
#[derive(Debug)]
pub struct SchemaBatch {
	pub(crate) rows: Mutex<SchemaBatchRows>,
}

impl Default for SchemaBatch {
	fn default() -> Self {
		Self {
			rows: Mutex::new(HashMap::new()),
		}
	}
}

impl SchemaBatch {
	/// Creates an empty batch.
	pub fn new() -> Self {
		Self::default()
	}

	/// Moves a private batch into this one, preserving operation order within each column family.
	/// Key/value buffers are retained; callers must append fragments in their logical order.
	pub fn append(&mut self, other: Self) {
		let rows = self.rows.get_mut().expect("RdbBatchAppend: poisoned lock");
		for (column_family, mut operations) in other.into_rows() {
			match rows.entry(column_family) {
				std::collections::hash_map::Entry::Vacant(entry) => {
					entry.insert(operations);
				}
				std::collections::hash_map::Entry::Occupied(mut entry) => entry.get_mut().append(&mut operations),
			}
		}
	}

	/// Adds an insert/update operation to the batch.
	pub fn put<S: Schema>(&self, key: &S::Key, value: &S::Value) -> AppResult<()> {
		let key = <S::Key as KeyCodec<S>>::encode_key(key)?;
		let value = <S::Value as ValueCodec<S>>::encode_value(value)?;
		self.rows
			.lock()
			.expect("RdbBatchPut: poisoned lock")
			.entry(Cow::Borrowed(S::COLUMN_FAMILY_NAME))
			.or_default()
			.push(WriteOp::Value { key, value });

		Ok(())
	}

	/// Adds a delete operation to the batch.
	pub fn delete<S: Schema>(&self, key: &S::Key) -> AppResult<()> {
		let key = <S::Key as KeyCodec<S>>::encode_key(key)?;
		self.rows
			.lock()
			.expect("RdbBatchDel: poisoned lock")
			.entry(Cow::Borrowed(S::COLUMN_FAMILY_NAME))
			.or_default()
			.push(WriteOp::Deletion { key });

		Ok(())
	}

	pub fn into_column_family_batches(self, column_family_groups: &[&[ColumnFamilyName]]) -> Vec<SchemaBatch> {
		let mut rows = self.rows.into_inner().expect("RdbBatchSplit: poisoned lock");
		let mut batches = Vec::with_capacity(column_family_groups.len());

		for column_families in column_family_groups {
			let batch = SchemaBatch::new();
			{
				let mut batch_rows = batch.rows.lock().expect("RdbBatchSplit: poisoned lock");
				for column_family in *column_families {
					if let Some(write_ops) = rows.remove(*column_family) {
						batch_rows.insert(Cow::Borrowed(*column_family), write_ops);
					}
				}
			}
			batches.push(batch);
		}

		debug_assert!(
			rows.is_empty(),
			"SchemaBatch contains column families that were not assigned to a target batch: {:?}",
			rows.keys().collect::<Vec<_>>()
		);
		batches
	}

	pub(crate) fn into_rows(self) -> SchemaBatchRows {
		self.rows.into_inner().expect("RdbBatchIntoRows: poisoned lock")
	}

	pub(crate) fn from_rows(rows: SchemaBatchRows) -> Self {
		Self { rows: Mutex::new(rows) }
	}
}
