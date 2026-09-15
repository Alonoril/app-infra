use super::{
	core::RksDB,
	schema::{KeyCodec, Schema, ValueCodec},
	utils::IntoDbResult,
};
use infra_core::result::AppResult;
use rocksdb::{DB, SnapshotWithThreadMode};

/// A typed read view pinned to one RocksDB sequence number.
pub struct RksDBSnapshot<'db> {
	db: &'db RksDB,
	snapshot: SnapshotWithThreadMode<'db, DB>,
}

impl RksDB {
	/// Creates a snapshot that observes all schemas at one RocksDB sequence number.
	pub fn snapshot(&self) -> RksDBSnapshot<'_> {
		RksDBSnapshot {
			db: self,
			snapshot: self.inner.snapshot(),
		}
	}
}

impl RksDBSnapshot<'_> {
	/// Reads one typed value from this snapshot.
	pub fn get<S: Schema>(&self, key: &S::Key) -> AppResult<Option<S::Value>> {
		let encoded_key = <S::Key as KeyCodec<S>>::encode_key(key)?;
		let cf_handle = self.db.get_cf_handle(S::COLUMN_FAMILY_NAME)?;
		self.snapshot
			.get_cf(&cf_handle, encoded_key)
			.into_db_res()?
			.map(|raw_value| <S::Value as ValueCodec<S>>::decode_value(&raw_value))
			.transpose()
	}

	/// Reads typed values in the same order as the input keys.
	pub fn multi_get<S: Schema>(&self, keys: &[S::Key]) -> AppResult<Vec<Option<S::Value>>> {
		let cf_handle = self.db.get_cf_handle(S::COLUMN_FAMILY_NAME)?;
		let encoded_keys = keys
			.iter()
			.map(|key| <S::Key as KeyCodec<S>>::encode_key(key))
			.collect::<AppResult<Vec<_>>>()?;
		self.snapshot
			.multi_get_cf(encoded_keys.iter().map(|key| (&cf_handle, key)))
			.into_iter()
			.map(|result| {
				result
					.into_db_res()?
					.map(|raw_value| <S::Value as ValueCodec<S>>::decode_value(&raw_value))
					.transpose()
			})
			.collect()
	}
}
