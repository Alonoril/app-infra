use super::{DurableWriteBatch, DurableWriteOp, IntoDbResult, RksDB};
use crate::errors::{self, RdbDetail};
use infra_core::result::AppResult;
use std::{
	collections::HashSet,
	time::{Duration, Instant},
};

/// Encoded logical bytes and native batch size are different from disk/WAL bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct DurableWriteStats {
	pub native_build: Duration,
	pub write: Duration,
	pub key_bytes: u64,
	pub value_bytes: u64,
	pub native_bytes: u64,
	pub puts: u64,
	pub deletes: u64,
	pub wal_enabled: bool,
	pub sync: bool,
}

/// A native batch bound to the database that resolved its column families.
/// Preparing it never writes state; `write` preserves the requested WAL/sync mode.
pub struct PreparedDurableWrite<'a> {
	db: &'a RksDB,
	batch: rocksdb::WriteBatch,
	stats: DurableWriteStats,
}

impl PreparedDurableWrite<'_> {
	pub fn write(mut self) -> AppResult<DurableWriteStats> {
		let mut options = rocksdb::WriteOptions::default();
		options.set_sync(self.stats.sync);
		options.disable_wal(!self.stats.wal_enabled);
		let write_started = Instant::now();
		self.db.inner.write_opt(self.batch, &options).into_db_res()?;
		self.stats.write = write_started.elapsed();
		Ok(self.stats)
	}
}

impl RksDB {
	pub fn write_durable_batch_sync(&self, batch: DurableWriteBatch) -> AppResult<()> {
		self.write_durable_batch_sync_with_stats(batch).map(|_| ())
	}

	pub fn write_durable_batch_sync_with_stats(&self, batch: DurableWriteBatch) -> AppResult<DurableWriteStats> {
		self.write_durable_batch_measured(batch, true, true)
	}

	pub fn prepare_durable_batch_sync(&self, batch: DurableWriteBatch) -> AppResult<PreparedDurableWrite<'_>> {
		self.prepare_durable_batch(batch, true, true)
	}

	/// Requires a complete, durable application recovery protocol. A successful write
	/// does not make unlogged memtables persistent or authorize deleting recovery records.
	pub fn write_durable_batch_unlogged(&self, batch: DurableWriteBatch) -> AppResult<()> {
		self.write_durable_batch_unlogged_with_stats(batch).map(|_| ())
	}

	pub fn write_durable_batch_unlogged_with_stats(&self, batch: DurableWriteBatch) -> AppResult<DurableWriteStats> {
		self.write_durable_batch_measured(batch, false, false)
	}

	fn write_durable_batch_measured(
		&self,
		batch: DurableWriteBatch,
		wal_enabled: bool,
		sync: bool,
	) -> AppResult<DurableWriteStats> {
		self.prepare_durable_batch(batch, wal_enabled, sync)?.write()
	}

	fn prepare_durable_batch(
		&self,
		batch: DurableWriteBatch,
		wal_enabled: bool,
		sync: bool,
	) -> AppResult<PreparedDurableWrite<'_>> {
		let started = Instant::now();
		let mut db_batch = rocksdb::WriteBatch::default();
		let mut column_families = HashSet::with_capacity(batch.column_families.len());
		let mut stats = DurableWriteStats {
			wal_enabled,
			sync,
			..Default::default()
		};
		for column_family_batch in batch.column_families {
			if !column_families.insert(column_family_batch.column_family.clone()) {
				return Err(errors::invalid_params(RdbDetail::ColumnFamily));
			}
			let cf_handle = self.get_cf_handle(&column_family_batch.column_family)?;
			for write_op in column_family_batch.operations {
				match write_op {
					DurableWriteOp::Value { key, value } => {
						stats.puts += 1;
						stats.key_bytes += key.len() as u64;
						stats.value_bytes += value.len() as u64;
						db_batch.put_cf(&cf_handle, key, value);
					}
					DurableWriteOp::Deletion { key } => {
						stats.deletes += 1;
						stats.key_bytes += key.len() as u64;
						db_batch.delete_cf(&cf_handle, key);
					}
				}
			}
		}
		stats.native_bytes = db_batch.size_in_bytes() as u64;
		stats.native_build = started.elapsed();
		Ok(PreparedDurableWrite {
			db: self,
			batch: db_batch,
			stats,
		})
	}
}
