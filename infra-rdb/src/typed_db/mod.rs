#![forbid(unsafe_code)]

#[macro_use]
pub mod schema;
mod batch;
mod core;
mod durable_batch;
mod iterator;
#[cfg(feature = "ttl")]
mod ttl;
mod utils;

// Re-export public types and traits
pub use batch::{ColumnFamilyName, SchemaBatch};
pub use core::RksDB;
pub use durable_batch::{DurableColumnFamilyBatch, DurableWriteBatch, DurableWriteOp};
use infra_rdb_cfg::RocksdbConfig;
pub use iterator::{ScanDirection, SchemaIterator};
pub use schema::Schema;
pub use utils::IntoDbResult;
pub(crate) use utils::write_options_from_config;

use crate::CfPost;
/// Type alias to `rocksdb::ReadOptions`. See [`rocksdb doc`](https://github.com/pingcap/rust-rocksdb/blob/master/src/rocksdb_options.rs)
pub use rocksdb::{
	BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, DEFAULT_COLUMN_FAMILY_NAME, Options,
	ReadOptions, SliceTransform,
};

const BYTES_PER_SYNC: u64 = 1024 * 1024;
const WAL_BYTES_PER_SYNC: u64 = 1024 * 1024;

pub fn gen_rocksdb_options(config: &RocksdbConfig, readonly: bool) -> Options {
	let mut db_opts = Options::default();
	db_opts.set_max_open_files(config.max_open_files);
	db_opts.set_max_total_wal_size(config.max_total_wal_size);
	db_opts.set_max_background_jobs(config.max_background_jobs);
	db_opts.increase_parallelism(config.increase_parallelism);
	db_opts.set_bytes_per_sync(BYTES_PER_SYNC);
	db_opts.set_wal_bytes_per_sync(WAL_BYTES_PER_SYNC);
	db_opts.set_allow_concurrent_memtable_write(true);
	db_opts.set_enable_pipelined_write(true);
	if !readonly {
		db_opts.create_if_missing(true);
		db_opts.create_missing_column_families(true);
	}

	db_opts
}

pub fn build_table_opts(rocksdb_config: &RocksdbConfig) -> (BlockBasedOptions, Cache) {
	let mut table_opts = BlockBasedOptions::default();
	table_opts.set_cache_index_and_filter_blocks(rocksdb_config.cache_index_and_filter_blocks);
	table_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
	table_opts.set_block_size(rocksdb_config.block_size as usize);

	let cache = Cache::new_lru_cache(rocksdb_config.block_cache_size as usize);
	table_opts.set_block_cache(&cache);

	table_opts.set_hybrid_ribbon_filter(10.0, 1);
	table_opts.set_format_version(5);

	// 返回 cache 是为了延长其生命周期（避免被 drop 后 table_opts 内引用失效）
	(table_opts, cache)
}

//     // bottommost 字典大小（max_dict_bytes）：比如 16KB / 32KB 常见
//     // 参数含义与 set_compression_options 相同；对 zstd 来说你主要关心 max_dict_bytes。
//     cf_opts.set_bottommost_compression_options(
//         0,   // w_bits（更多是 zlib 场景）
//         0,   // level（更多是 zlib 场景）
//         0,   // strategy（更多是 zlib 场景）
//         32 * 1024, // max_dict_bytes：字典最大大小（示例 32KiB）
//         true,      // enabled：必须 true 才会启用 bottommost 配置
//     );
//
//     // zstd 训练数据上限（train_bytes）：建议从 0 或几十/几百 KB 起步逐渐调
//     // train_bytes 越大，压缩率可能更好，但训练/内存开销越高
//     cf_opts.set_bottommost_zstd_max_train_bytes(256 * 1024, true);

pub fn build_cfds_with_post(
	rdb_cfg: &RocksdbConfig,
	cfs: &[ColumnFamilyName],
	post: CfPost,
) -> Vec<ColumnFamilyDescriptor> {
	let (table_opts, _cache) = build_table_opts(rdb_cfg);

	let mut cfds = Vec::with_capacity(cfs.len());
	for &cf_name in cfs {
		let mut cf_opts = Options::default();

		// bottommost ZSTD
		cf_opts.set_bottommost_compression_type(DBCompressionType::Zstd);
		cf_opts.set_bottommost_zstd_max_train_bytes(0, true);

		cf_opts.set_level_compaction_dynamic_level_bytes(true);
		cf_opts.set_block_based_table_factory(&table_opts);

		// 写入缓冲区：增大 memtable，减少频繁 flush
		cf_opts.set_write_buffer_size(rdb_cfg.write_buffer_size); // 256MB 起步
		cf_opts.set_max_write_buffer_number(rdb_cfg.max_write_buffer_number);
		cf_opts.set_min_write_buffer_number_to_merge(rdb_cfg.min_write_buffer_number_to_merge);

		// 放宽 L0 文件触发阈值，避免过早 write stall
		cf_opts.set_level_zero_file_num_compaction_trigger(rdb_cfg.level_zero_file_num_compaction_trigger);
		cf_opts.set_level_zero_slowdown_writes_trigger(rdb_cfg.level0_slowdown_writes_trigger);
		cf_opts.set_level_zero_stop_writes_trigger(rdb_cfg.level0_stop_writes_trigger);

		// 增大 SST 文件和 level 容量
		cf_opts.set_target_file_size_base(256 * 1024 * 1024);
		cf_opts.set_max_bytes_for_level_base(1024 * 1024 * 1024);

		// 写入优先时，底层再压缩；L0/L1 可不压缩
		cf_opts.set_compression_per_level(&[
			DBCompressionType::None,
			DBCompressionType::None,
			DBCompressionType::Lz4,
			DBCompressionType::Lz4,
			DBCompressionType::Lz4,
			DBCompressionType::Lz4,
			DBCompressionType::Lz4,
		]);

		// L1~Ln LZ4
		// cf_opts.set_compression_type(DBCompressionType::Lz4);

		post(cf_name, &mut cf_opts);

		cfds.push(ColumnFamilyDescriptor::new((*cf_name).to_string(), cf_opts));
	}
	cfds
}

//use rocksdb::{DB, Options, DBCompressionType};
//
// let mut opts = Options::default();
// opts.create_if_missing(true);
//
// // 1. 提高后台 flush / compaction 并发
// opts.increase_parallelism(num_cpus::get() as i32);
//
// // 2. 写入缓冲区：增大 memtable，减少频繁 flush
// opts.set_write_buffer_size(256 * 1024 * 1024); // 256MB 起步
// opts.set_max_write_buffer_number(6);
// opts.set_min_write_buffer_number_to_merge(1);
//
// // 3. 放宽 L0 文件触发阈值，避免过早 write stall
// opts.set_level_zero_file_num_compaction_trigger(8);
// opts.set_level_zero_slowdown_writes_trigger(32);
// opts.set_level_zero_stop_writes_trigger(64);
//
// // 4. 增大 SST 文件和 level 容量
// opts.set_target_file_size_base(256 * 1024 * 1024);
// opts.set_max_bytes_for_level_base(1024 * 1024 * 1024);
//
// // 5. 写入优先时，底层再压缩；L0/L1 可不压缩
// opts.set_compression_per_level(&[
//     DBCompressionType::None,
//     DBCompressionType::None,
//     DBCompressionType::Lz4,
//     DBCompressionType::Lz4,
//     DBCompressionType::Lz4,
//     DBCompressionType::Lz4,
//     DBCompressionType::Lz4,
// ]);
//
// let db = DB::open(&opts, "./indexer_rocksdb")?;
