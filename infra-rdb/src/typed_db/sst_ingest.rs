use super::{DurableWriteBatch, DurableWriteOp, DurableWriteStats, IntoDbResult, RksDB};
use crate::errors::{self, RdbDetail};
use infra_core::result::AppResult;
use rocksdb::{IngestExternalFileOptions, Options, SstFileWriter};
use std::{collections::HashSet, time::Instant};

impl RksDB {
	/// Builds sorted SST files beside the database and ingests them without WAL writes.
	/// A batch spanning column families is not atomic: callers must replay an
	/// incomplete block and publish their checkpoint only after every ingest succeeds.
	pub fn ingest_durable_batch_sst_with_stats(&self, batch: DurableWriteBatch) -> AppResult<DurableWriteStats> {
		let staging_directory = tempfile::Builder::new()
			.prefix(".sst-ingest-")
			.tempdir_in(self.inner.path())
			.into_db_res()?;
		let mut seen_column_families = HashSet::with_capacity(batch.column_families.len());
		let mut stats = DurableWriteStats::default();
		for (ordinal, mut family) in batch.column_families.into_iter().enumerate() {
			let build_started = Instant::now();
			if !seen_column_families.insert(family.column_family.clone()) {
				return Err(errors::invalid_params(RdbDetail::ColumnFamily));
			}
			let handle = self.get_cf_handle(&family.column_family)?;
			if family.operations.is_empty() {
				continue;
			}
			compact_last_operations(&mut family.operations);
			let file_path = staging_directory.path().join(format!("{ordinal}.sst"));
			let options = Options::default();
			let mut writer = SstFileWriter::create(&options);
			writer.open(&file_path).into_db_res()?;
			for operation in family.operations {
				match operation {
					DurableWriteOp::Value { key, value } => {
						stats.key_bytes += key.len() as u64;
						stats.value_bytes += value.len() as u64;
						stats.puts += 1;
						writer.put(key, value).into_db_res()?;
					}
					DurableWriteOp::Deletion { key } => {
						stats.key_bytes += key.len() as u64;
						stats.deletes += 1;
						writer.delete(key).into_db_res()?;
					}
				}
			}
			writer.finish().into_db_res()?;
			stats.native_bytes += writer.file_size();
			stats.native_build += build_started.elapsed();
			let mut ingest_options = IngestExternalFileOptions::default();
			ingest_options.set_move_files(true);
			let ingest_started = Instant::now();
			self.inner
				.ingest_external_file_cf_opts(&handle, &ingest_options, vec![file_path])
				.into_db_res()?;
			stats.write += ingest_started.elapsed();
		}
		Ok(stats)
	}
}

fn operation_key(operation: &DurableWriteOp) -> &[u8] {
	match operation {
		DurableWriteOp::Value { key, .. } | DurableWriteOp::Deletion { key } => key,
	}
}

fn compact_last_operations(operations: &mut Vec<DurableWriteOp>) {
	operations.sort_by(|left, right| operation_key(left).cmp(operation_key(right)));
	let mut retained = 0;
	for index in 0..operations.len() {
		if retained > 0 && operation_key(&operations[retained - 1]) == operation_key(&operations[index]) {
			operations.swap(retained - 1, index);
		} else {
			operations.swap(retained, index);
			retained += 1;
		}
	}
	operations.truncate(retained);
}
