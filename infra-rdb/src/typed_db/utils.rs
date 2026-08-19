use crate::{DbResult, errors};
use infra_core::result::AppError;
use infra_rdb_cfg::WriteOptionsConfig;
use std::{io::Error, path::Path};

#[derive(Debug)]
pub(crate) enum OpenMode<'a> {
	ReadWrite,
	ReadOnly,
	Secondary(&'a Path),
}

/// For now we always use synchronous writes. This makes sure that once the operation returns
/// `Ok(())` the data is persisted even if the machine crashes. In the future we might consider
/// selectively turning this off for some non-critical writes to improve performance.
pub(crate) fn default_write_options() -> rocksdb::WriteOptions {
	let mut opts = rocksdb::WriteOptions::default();
	opts.set_sync(true);
	opts
}

pub(crate) fn write_options_from_config(config: WriteOptionsConfig) -> rocksdb::WriteOptions {
	let mut opts = rocksdb::WriteOptions::default();
	opts.set_sync(config.sync());
	opts.disable_wal(config.disable_wal());
	opts
}

pub(crate) trait DeUnc: AsRef<Path> {
	fn de_unc(&self) -> &Path {
		// `dunce` is needed to "de-UNC" because rocksdb doesn't take Windows UNC paths like `\\?\C:\`
		dunce::simplified(self.as_ref())
	}
}

impl<T> DeUnc for T where T: AsRef<Path> {}

fn to_db_err(rocksdb_err: rocksdb::Error) -> AppError {
	errors::from_rocksdb_error(rocksdb_err)
}

pub trait IntoDbResult<T> {
	fn into_db_res(self) -> DbResult<T>;
}

impl<T> IntoDbResult<T> for Result<T, rocksdb::Error> {
	fn into_db_res(self) -> DbResult<T> {
		self.map_err(to_db_err)
	}
}

impl<T> IntoDbResult<T> for Result<T, Error> {
	fn into_db_res(self) -> DbResult<T> {
		self.map_err(from_io_err)
	}
}

fn from_io_err(io_err: Error) -> AppError {
	errors::from_io_error(io_err)
}
