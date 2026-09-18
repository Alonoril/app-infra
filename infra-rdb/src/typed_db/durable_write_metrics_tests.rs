use super::RksDB;
use crate::typed_db::{DurableColumnFamilyBatch, DurableWriteBatch, DurableWriteOp};
use rocksdb::Options;

#[test]
fn durable_write_metrics_count_actual_operations_and_preserve_order() {
	let directory = tempfile::tempdir().unwrap();
	let mut options = Options::default();
	options.create_if_missing(true);
	let db = RksDB::open(directory.path().join("db"), "metrics", vec!["default"], &options).unwrap();
	let batch = DurableWriteBatch {
		column_families: vec![DurableColumnFamilyBatch {
			column_family: "default".into(),
			operations: vec![
				DurableWriteOp::Value {
					key: b"k".to_vec(),
					value: b"first".to_vec(),
				},
				DurableWriteOp::Deletion { key: b"k".to_vec() },
				DurableWriteOp::Value {
					key: b"k".to_vec(),
					value: b"last".to_vec(),
				},
			],
		}],
	};
	let stats = db.write_durable_batch_sync_with_stats(batch).unwrap();
	assert_eq!(stats.puts, 2);
	assert_eq!(stats.deletes, 1);
	assert_eq!(stats.key_bytes, 3);
	assert_eq!(stats.value_bytes, 9);
	assert!(stats.wal_enabled && stats.sync);
	assert!(stats.native_bytes >= 12);
	assert_eq!(db.inner.get(b"k").unwrap().unwrap(), b"last");
	let stats = db
		.write_durable_batch_unlogged_with_stats(DurableWriteBatch::default())
		.unwrap();
	assert_eq!(stats.puts + stats.deletes, 0);
	assert!(!stats.wal_enabled && !stats.sync);
}
