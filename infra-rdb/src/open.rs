use crate::typed_db::{ColumnFamilyName, RksDB, build_cfds_with_post, gen_rocksdb_options, write_options_from_config};
use std::{path::PathBuf, time::Instant};
use tracing::info;

use infra_core::result::AppResult;
use infra_rdb_cfg::{RksDbDirPaths, RocksdbConfig};
use rocksdb::{ColumnFamilyDescriptor, Options};

pub type DbResult<T> = AppResult<T>;

pub type CfPost = fn(ColumnFamilyName, &mut Options);

pub trait OpenRocksDB {
	fn new(path: PathBuf, name: &str, db_config: &RocksdbConfig, readonly: bool) -> AppResult<Self>
	where
		Self: Sized,
	{
		let db = Self::open_rocksdb(path, name, db_config, readonly)?;
		Self::new_inner(db)
	}

	fn new_inner(db: RksDB) -> AppResult<Self>
	where
		Self: Sized;

	fn get_db_column_families() -> Vec<ColumnFamilyName>;

	fn get_db_column_families_ops_ttl() -> Vec<ColumnFamilyName> {
		#[cfg(feature = "ttl")]
		{
			let mut cfs = Self::get_db_column_families();
			cfs.extend(RksDB::get_ttl_column_families());
			cfs
		}

		#[cfg(not(feature = "ttl"))]
		{
			Self::get_db_column_families()
		}
	}

	fn cf_opts_post_processor() -> CfPost {
		noop_cf_post
	}

	fn gen_db_cfds(rocksdb_config: &RocksdbConfig) -> Vec<ColumnFamilyDescriptor> {
		let post = Self::cf_opts_post_processor();
		build_cfds_with_post(rocksdb_config, &Self::get_db_column_families_ops_ttl(), post)
	}

	fn open_rocksdb(path: PathBuf, name: &str, db_config: &RocksdbConfig, readonly: bool) -> AppResult<RksDB> {
		let started_at = Instant::now();
		let cfds = Self::gen_db_cfds(db_config);

		let db = if readonly {
			RksDB::open_cf_readonly_with_write_options(
				&gen_rocksdb_options(db_config, true),
				path.clone(),
				name,
				cfds,
				write_options_from_config(db_config.write_options),
			)?
		} else {
			RksDB::open_cf_with_write_options(
				&gen_rocksdb_options(db_config, false),
				path.clone(),
				name,
				cfds,
				write_options_from_config(db_config.write_options),
			)?
		};

		info!("Database {name} opened in {:?} at {path:?}!", started_at.elapsed());
		Ok(db)
	}

	fn get_db_path(db_paths: RksDbDirPaths) -> PathBuf;
}

#[inline]
pub fn noop_cf_post(_: ColumnFamilyName, _: &mut Options) {}
