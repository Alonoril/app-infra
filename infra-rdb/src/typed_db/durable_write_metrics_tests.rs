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

#[test]
fn prepared_durable_batch_does_not_write_until_commit() {
	let directory = tempfile::tempdir().unwrap();
	let mut options = Options::default();
	options.create_if_missing(true);
	let db = RksDB::open(directory.path().join("db"), "prepared", vec!["default"], &options).unwrap();
	let batch = DurableWriteBatch {
		column_families: vec![DurableColumnFamilyBatch {
			column_family: "default".into(),
			operations: vec![DurableWriteOp::Value {
				key: b"prepared".to_vec(),
				value: b"durable".to_vec(),
			}],
		}],
	};
	let prepared = db.prepare_durable_batch_sync(batch).unwrap();
	assert!(db.inner.get(b"prepared").unwrap().is_none());
	let stats = prepared.write().unwrap();
	assert!(stats.wal_enabled && stats.sync);
	assert_eq!(stats.puts, 1);
	assert_eq!(db.inner.get(b"prepared").unwrap().unwrap(), b"durable");
}

#[test]
fn sst_ingest_sorts_keys_and_keeps_last_operation() {
	let directory = tempfile::tempdir().unwrap();
	let mut options = Options::default();
	options.create_if_missing(true);
	let db = RksDB::open(directory.path().join("db"), "sst-ingest", vec!["default"], &options).unwrap();
	db.inner.put(b"z", b"persisted").unwrap();
	let batch = DurableWriteBatch {
		column_families: vec![DurableColumnFamilyBatch {
			column_family: "default".into(),
			operations: vec![
				DurableWriteOp::Value {
					key: b"z".to_vec(),
					value: b"old".to_vec(),
				},
				DurableWriteOp::Value {
					key: b"a".to_vec(),
					value: b"first".to_vec(),
				},
				DurableWriteOp::Deletion { key: b"z".to_vec() },
				DurableWriteOp::Value {
					key: b"a".to_vec(),
					value: b"last".to_vec(),
				},
				DurableWriteOp::Value {
					key: b"m".to_vec(),
					value: b"middle".to_vec(),
				},
			],
		}],
	};

	let stats = db.ingest_durable_batch_sst_with_stats(batch).unwrap();

	assert_eq!(db.inner.get(b"a").unwrap().unwrap(), b"last");
	assert_eq!(db.inner.get(b"m").unwrap().unwrap(), b"middle");
	assert!(db.inner.get(b"z").unwrap().is_none());
	assert_eq!((stats.puts, stats.deletes), (2, 1));
	assert!(!stats.wal_enabled && !stats.sync);
}
