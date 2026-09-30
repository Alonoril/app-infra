use super::{IntoDbResult, RksDB};
use crate::errors::RdbErr;
use infra_core::{err, result::AppResult};

impl RksDB {
	/// Checks every existing column family without decoding or collecting values.
	/// Intended for startup guards and offline maintenance, not hot paths.
	pub fn has_any_rows(&self) -> AppResult<bool> {
		let families = rocksdb::DB::list_cf(&rocksdb::Options::default(), self.inner.path()).into_db_res()?;
		for family in families {
			let handle = self.get_cf_handle(&family)?;
			let mut iterator = self.inner.raw_iterator_cf(&handle);
			iterator.seek_to_first();
			iterator.status().into_db_res()?;
			if iterator.valid() {
				return Ok(true);
			}
		}
		Ok(false)
	}

	/// Physically retires a column family while the caller owns the database.
	/// The application must validate an explicit allowlist before calling this.
	pub fn drop_column_family_if_exists(&mut self, family: &str) -> AppResult<bool> {
		if family == rocksdb::DEFAULT_COLUMN_FAMILY_NAME {
			return err!(RdbErr::InvalidParams, "the default column family cannot be retired");
		}
		if self.inner.cf_handle(family).is_none() {
			return Ok(false);
		}
		self.inner.drop_cf(family).into_db_res()?;
		Ok(true)
	}
}

#[cfg(test)]
mod tests {
	use super::RksDB;
	use rocksdb::Options;

	#[test]
	fn physical_retirement_preserves_other_families_and_survives_reopen() {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("db");
		let mut options = Options::default();
		options.create_if_missing(true);
		options.create_missing_column_families(true);
		let mut db = RksDB::open(&path, "retirement", vec!["default", "legacy", "active"], &options).unwrap();
		assert!(!db.has_any_rows().unwrap());
		db.inner
			.put_cf(db.inner.cf_handle("legacy").unwrap(), b"key", b"obsolete")
			.unwrap();
		assert!(db.has_any_rows().unwrap());
		db.inner
			.put_cf(db.inner.cf_handle("active").unwrap(), b"key", b"retained")
			.unwrap();
		assert!(db.drop_column_family_if_exists("legacy").unwrap());
		assert!(!db.drop_column_family_if_exists("legacy").unwrap());
		drop(db);
		options.create_missing_column_families(false);
		let reopened = RksDB::open(&path, "retirement", vec!["default", "active"], &options).unwrap();
		assert!(reopened.inner.cf_handle("legacy").is_none());
		assert_eq!(
			reopened
				.inner
				.get_cf(reopened.inner.cf_handle("active").unwrap(), b"key")
				.unwrap()
				.unwrap(),
			b"retained"
		);
	}

	#[test]
	fn default_family_cannot_be_retired_and_participates_in_empty_guard() {
		let directory = tempfile::tempdir().unwrap();
		let mut options = Options::default();
		options.create_if_missing(true);
		let mut db = RksDB::open(directory.path().join("db"), "default-guard", vec!["default"], &options).unwrap();
		db.inner.put(b"key", b"value").unwrap();
		assert!(db.has_any_rows().unwrap());
		assert!(db.drop_column_family_if_exists("default").is_err());
		assert_eq!(db.inner.get(b"key").unwrap().unwrap(), b"value");
	}
}
