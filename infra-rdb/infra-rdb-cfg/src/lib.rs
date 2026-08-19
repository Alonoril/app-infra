use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
/// Throughput-oriented write defaults:
///    sync=false
///    disableWAL=true
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct WriteOptionsConfig {
	sync: bool,
	disable_wal: bool,
}

impl Default for WriteOptionsConfig {
	fn default() -> Self {
		Self {
			sync: true,
			disable_wal: false,
		}
	}
}

impl WriteOptionsConfig {
	pub fn new_with_disable_wal() -> Self {
		Self {
			sync: false,
			disable_wal: true,
		}
	}

	pub fn sync(&self) -> bool {
		self.sync
	}

	pub fn disable_wal(&self) -> bool {
		self.disable_wal
	}
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DbPathConfig {
	pub rks_db_path: Option<PathBuf>,
}

// increase_parallelism(cpu 核数)
//    write_buffer_size=256MB
//    max_write_buffer_number=6
//    level0_slowdown_writes_trigger=32
//    level0_stop_writes_trigger=64
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RocksdbConfig {
	/// Maximum number of files open by RocksDB at one time
	pub max_open_files: i32,
	/// Maximum size of the RocksDB write ahead log (WAL)
	pub max_total_wal_size: u64,
	/// Maximum number of background threads for Rocks DB
	pub max_background_jobs: i32,
	/// Block cache size for Rocks DB
	pub block_cache_size: u64,
	/// Block size for Rocks DB
	pub block_size: u64,
	/// Whether cache index and filter blocks into block cache.
	pub cache_index_and_filter_blocks: bool,
	/// Increase parallelism to number of CPU cores
	pub increase_parallelism: i32,
	/// Write buffer size for RocksDB
	pub write_buffer_size: usize,
	/// Maximum number of write buffers to be merged per compaction
	pub max_write_buffer_number: i32,
	/// Minimum number of write buffers that will be merged at the same time
	pub min_write_buffer_number_to_merge: i32,
	/// level0_file_num_compaction_trigger=32
	pub level_zero_file_num_compaction_trigger: i32,
	/// level0_slowdown_writes_trigger=32
	pub level0_slowdown_writes_trigger: i32,
	/// level0_stop_writes_trigger=64
	pub level0_stop_writes_trigger: i32,
	/// Write options for RocksDB
	pub write_options: WriteOptionsConfig,
}

impl Default for RocksdbConfig {
	fn default() -> Self {
		Self {
			// Allow db to close old sst files, saving memory.
			max_open_files: 5000,
			// For now we set the max total WAL size to be 1G. This config can be useful when column
			// families are updated at non-uniform frequencies.
			max_total_wal_size: 1u64 << 30,
			// This includes threads for flashing and compaction. Rocksdb will decide the # of
			// threads to use internally.
			max_background_jobs: 16,
			// Default block cache size is 8MB,
			block_cache_size: 8 * (1u64 << 20),
			// Default block size is 4KB,
			block_size: 4 * (1u64 << 10),
			// Whether cache index and filter blocks into block cache.
			cache_index_and_filter_blocks: false,
			// Increase parallelism to number of CPU cores, default to 4
			increase_parallelism: 4,
			// Write buffer size for RocksDB default to 256MB
			write_buffer_size: 256 * 1024 * 1024,
			// L0 layer file count threshold. When the L0 file count reaches this value→ RocksDB actively slows down and limits flow (slowdown), but stops before reaching that threshold.
			max_write_buffer_number: 6,
			min_write_buffer_number_to_merge: 1,
			level_zero_file_num_compaction_trigger: 8,
			level0_slowdown_writes_trigger: 32,
			// L0 layer SST file count threshold; when L0 reaches this threshold, the Write Stall is triggered to block user writes
			level0_stop_writes_trigger: 64,
			write_options: WriteOptionsConfig::new_with_disable_wal(),
		}
	}
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RocksdbConfigs {
	pub rks_db_config: RocksdbConfig,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RksdbConfig {
	/// Top level directory to store the RocksDB
	pub dir: PathBuf,
	/// Subdirectory for handler in tests only
	#[serde(skip)]
	data_dir: PathBuf,
	/// Rocksdb-specific configurations
	pub rocksdb_configs: RocksdbConfigs,
}

#[derive(Clone)]
pub struct RksDbDirPaths {
	default_path: PathBuf,
	rksdb_path: Option<PathBuf>,
}

impl Default for RksdbConfig {
	fn default() -> RksdbConfig {
		RksdbConfig {
			dir: PathBuf::from("rks_db"),
			data_dir: PathBuf::from("/opt/app/data"),
			rocksdb_configs: RocksdbConfigs::default(),
		}
	}
}

impl RksdbConfig {
	pub fn dir(&self) -> PathBuf {
		if self.dir.is_relative() {
			self.data_dir.join(&self.dir)
		} else {
			self.dir.clone()
		}
	}

	pub fn get_dir_paths(&self) -> RksDbDirPaths {
		let default_dir = self.dir();
		RksDbDirPaths::new(default_dir, None)
	}

	pub fn set_data_dir(&mut self, data_dir: PathBuf) {
		self.data_dir = data_dir;
	}
}

impl RksDbDirPaths {
	pub fn default_root_path(&self) -> &PathBuf {
		&self.default_path
	}

	pub fn rdb_root_path(&self) -> &PathBuf {
		if let Some(rdb_path) = self.rksdb_path.as_ref() {
			rdb_path
		} else {
			&self.default_path
		}
	}

	pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
		Self {
			default_path: path.as_ref().to_path_buf(),
			rksdb_path: None,
		}
	}

	fn new(default_path: PathBuf, rks_db_path: Option<PathBuf>) -> Self {
		Self {
			default_path,
			rksdb_path: rks_db_path,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::WriteOptionsConfig;

	#[test]
	fn write_options_default_is_throughput_oriented() {
		let config = WriteOptionsConfig::new_with_disable_wal();

		assert!(!config.sync());
		assert!(config.disable_wal());
	}
}
