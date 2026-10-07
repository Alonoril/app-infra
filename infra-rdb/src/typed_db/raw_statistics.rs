use super::{IntoDbResult, RksDB};
use crate::errors::RdbErr;
use infra_core::{err, ok_or_logged, result::AppResult};

/// Uncompressed application key/value lengths, excluding RocksDB internal keys and file overhead.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RawColumnFamilyStats {
	pub entries: u64,
	pub key_bytes: u64,
	pub value_bytes: u64,
	/// True only when the iterator reached the end of the family.
	pub complete: bool,
}

impl RksDB {
	/// Streams raw entries without decoding, copying values or populating the block cache.
	/// `Some(limit)` samples the first keys in byte order; `None` scans the whole family.
	/// A prefix sample is not a random sample and must not be presented as an exact total.
	pub fn raw_column_family_stats(&self, family: &str, limit: Option<u64>) -> AppResult<RawColumnFamilyStats> {
		if limit == Some(0) {
			return err!(RdbErr::InvalidParams, "raw statistics limit must be positive");
		}
		let handle = self.get_cf_handle(family)?;
		let mut read_options = rocksdb::ReadOptions::default();
		read_options.fill_cache(false);
		let mut iterator = self.inner.raw_iterator_cf_opt(&handle, read_options);
		iterator.seek_to_first();
		let mut statistics = RawColumnFamilyStats::default();
		while statistics.entries < limit.unwrap_or(u64::MAX) {
			let Some((key, value)) = iterator.item() else { break };
			statistics.entries += 1;
			statistics.key_bytes = statistics
				.key_bytes
				.checked_add(key.len() as u64)
				.ok_or_else(ok_or_logged!(
					RdbErr::InvalidParams,
					format!("cf={family}, count=key_bytes, entries={}", statistics.entries)
				))?;
			statistics.value_bytes =
				statistics
					.value_bytes
					.checked_add(value.len() as u64)
					.ok_or_else(ok_or_logged!(
						RdbErr::InvalidParams,
						format!("cf={family}, count=value_bytes, entries={}", statistics.entries)
					))?;
			iterator.next();
		}
		iterator.status().into_db_res()?;
		statistics.complete = !iterator.valid();
		Ok(statistics)
	}
}

#[cfg(test)]
mod tests {
	use crate::typed_db::{Options, RksDB};

	fn open_test_db(path: &std::path::Path) -> RksDB {
		let mut options = Options::default();
		options.create_if_missing(true);
		options.create_missing_column_families(true);
		RksDB::open(path, "raw-statistics", vec!["default", "legacy"], &options).unwrap()
	}

	#[test]
	fn prefix_sample_stops_at_limit_without_decoding_and_reports_truncation() {
		let directory = tempfile::tempdir().unwrap();
		let db = open_test_db(&directory.path().join("db"));
		let family = db.inner.cf_handle("legacy").unwrap();
		for (key, value) in [(b"a".as_slice(), &[255, 254][..]), (b"bb", &[0]), (b"ccc", &[1, 2, 3])] {
			db.inner.put_cf(&family, key, value).unwrap();
		}

		let sample = db.raw_column_family_stats("legacy", Some(2)).unwrap();
		assert_eq!(sample.entries, 2);
		assert_eq!(sample.key_bytes, 3);
		assert_eq!(sample.value_bytes, 3);
		assert!(!sample.complete);
		let exact = db.raw_column_family_stats("legacy", None).unwrap();
		assert_eq!((exact.entries, exact.key_bytes, exact.value_bytes), (3, 6, 6));
		assert!(exact.complete);
	}

	#[test]
	fn empty_and_exact_limit_samples_are_complete() {
		let directory = tempfile::tempdir().unwrap();
		let db = open_test_db(&directory.path().join("db"));
		let empty = db.raw_column_family_stats("default", Some(1)).unwrap();
		assert_eq!((empty.entries, empty.key_bytes, empty.value_bytes), (0, 0, 0));
		assert!(empty.complete);
		db.inner.put(b"key", b"").unwrap();
		let exact_limit = db.raw_column_family_stats("default", Some(1)).unwrap();
		assert_eq!(
			(exact_limit.entries, exact_limit.key_bytes, exact_limit.value_bytes),
			(1, 3, 0)
		);
		assert!(exact_limit.complete);
	}

	#[test]
	fn raw_statistics_reject_zero_limit_and_missing_family() {
		let directory = tempfile::tempdir().unwrap();
		let db = open_test_db(&directory.path().join("db"));
		assert!(db.raw_column_family_stats("legacy", Some(0)).is_err());
		assert!(db.raw_column_family_stats("missing", None).is_err());
	}

	#[test]
	fn read_only_statistics_include_legacy_families_without_application_codecs() {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("db");
		let db = open_test_db(&path);
		db.inner
			.put_cf(db.inner.cf_handle("legacy").unwrap(), &[255], &[254, 253])
			.unwrap();
		db.flush_cf("legacy").unwrap();
		drop(db);
		let reader = RksDB::open_cf_readonly(&Options::default(), &path, "legacy-report", Vec::new()).unwrap();
		let exact = reader.raw_column_family_stats("legacy", None).unwrap();
		assert_eq!((exact.entries, exact.key_bytes, exact.value_bytes), (1, 1, 2));
		assert!(exact.complete);
	}
}
