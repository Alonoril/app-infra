// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use byteorder::{LittleEndian, ReadBytesExt};
use infra_core::result::AppResult;
use infra_rdb::{
	define_schema,
	typed_db::{
		ColumnFamilyName, DurableColumnFamilyBatch, DurableWriteBatch, DurableWriteOp, IntoDbResult, RksDB,
		SchemaBatch,
		schema::{KeyCodec, Schema, ValueCodec},
	},
};
use rocksdb::{ColumnFamilyDescriptor, DEFAULT_COLUMN_FAMILY_NAME};

// Creating two wallets that share exactly the same structure but are stored in different column
// families. Also note that the key and value are of the same type `TestField`. By implementing
// both the `KeyCodec<>` and `ValueCodec<>` traits for both wallets, we are able to use it
// everywhere.
define_schema!(TestSchema1, TestField, TestField, "TestCF1");
define_schema!(TestSchema2, TestField, TestField, "TestCF2");

#[derive(Debug, Eq, PartialEq)]
struct TestField(u32);

impl TestField {
	fn to_bytes(&self) -> Vec<u8> {
		self.0.to_le_bytes().to_vec()
	}

	fn from_bytes(data: &[u8]) -> AppResult<Self> {
		let mut reader = std::io::Cursor::new(data);
		Ok(TestField(reader.read_u32::<LittleEndian>().into_db_res()?))
	}
}

impl KeyCodec<TestSchema1> for TestField {
	fn encode_key(&self) -> AppResult<Vec<u8>> {
		Ok(self.to_bytes())
	}

	fn decode_key(data: &[u8]) -> AppResult<Self> {
		Ok(Self::from_bytes(data)?)
	}
}

impl ValueCodec<TestSchema1> for TestField {
	fn encode_value(&self) -> AppResult<Vec<u8>> {
		Ok(self.to_bytes())
	}

	fn decode_value(data: &[u8]) -> AppResult<Self> {
		Ok(Self::from_bytes(data)?)
	}
}

impl KeyCodec<TestSchema2> for TestField {
	fn encode_key(&self) -> AppResult<Vec<u8>> {
		Ok(self.to_bytes())
	}

	fn decode_key(data: &[u8]) -> AppResult<Self> {
		Ok(Self::from_bytes(data)?)
	}
}

impl ValueCodec<TestSchema2> for TestField {
	fn encode_value(&self) -> AppResult<Vec<u8>> {
		Ok(self.to_bytes())
	}

	fn decode_value(data: &[u8]) -> AppResult<Self> {
		Ok(Self::from_bytes(data)?)
	}
}

fn get_column_families() -> Vec<ColumnFamilyName> {
	vec![
		DEFAULT_COLUMN_FAMILY_NAME,
		TestSchema1::COLUMN_FAMILY_NAME,
		TestSchema2::COLUMN_FAMILY_NAME,
	]
}

fn get_cfds() -> Vec<ColumnFamilyDescriptor> {
	get_column_families()
		.iter()
		.map(|cf_name| ColumnFamilyDescriptor::new(*cf_name, rocksdb::Options::default()))
		.collect()
}

fn open_db(dir: &aptos_temppath::TempPath) -> RksDB {
	let mut db_opts = rocksdb::Options::default();
	db_opts.create_if_missing(true);
	db_opts.create_missing_column_families(true);
	RksDB::open(dir.path(), "test", get_column_families(), &db_opts).expect("Failed to open DB.")
}

fn open_db_read_only(dir: &aptos_temppath::TempPath) -> RksDB {
	RksDB::open_cf_readonly(&rocksdb::Options::default(), dir.path(), "test", get_cfds()).expect("Failed to open DB.")
}

fn open_db_as_secondary(dir: &aptos_temppath::TempPath, dir_sec: &aptos_temppath::TempPath) -> RksDB {
	RksDB::open_cf_as_secondary(
		&rocksdb::Options::default(),
		dir.path(),
		dir_sec.path(),
		"test",
		get_cfds(),
	)
	.expect("Failed to open DB.")
}

struct TestDB {
	_tmpdir: aptos_temppath::TempPath,
	db: RksDB,
}

impl TestDB {
	fn new() -> Self {
		let tmpdir = aptos_temppath::TempPath::new();
		let db = open_db(&tmpdir);

		TestDB { _tmpdir: tmpdir, db }
	}
}

impl std::ops::Deref for TestDB {
	type Target = RksDB;

	fn deref(&self) -> &Self::Target {
		&self.db
	}
}

#[test]
fn durable_batch_round_trip_preserves_put_delete_and_is_idempotent() {
	let db = TestDB::new();
	db.put::<TestSchema1>(&TestField(2), &TestField(22)).unwrap();
	let batch = SchemaBatch::new();
	batch.put::<TestSchema1>(&TestField(1), &TestField(11)).unwrap();
	batch.delete::<TestSchema1>(&TestField(2)).unwrap();

	let encoded = bcs::to_bytes(&DurableWriteBatch::from_schema_batch(batch)).unwrap();
	let durable: DurableWriteBatch = bcs::from_bytes(&encoded).unwrap();
	db.write_durable_batch(durable.clone()).unwrap();
	db.write_durable_batch(durable).unwrap();

	assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), Some(TestField(11)));
	assert_eq!(db.get::<TestSchema1>(&TestField(2)).unwrap(), None);
}

#[test]
fn durable_batch_rejects_duplicate_column_families() {
	let batch = DurableWriteBatch {
		column_families: vec![
			DurableColumnFamilyBatch {
				column_family: TestSchema1::COLUMN_FAMILY_NAME.to_owned(),
				operations: vec![DurableWriteOp::Value {
					key: vec![1],
					value: vec![11],
				}],
			},
			DurableColumnFamilyBatch {
				column_family: TestSchema1::COLUMN_FAMILY_NAME.to_owned(),
				operations: vec![DurableWriteOp::Deletion { key: vec![2] }],
			},
		],
	};

	assert!(batch.into_schema_batch().is_err());
}

#[test]
fn durable_batch_sync_applies_operations() {
	let db = TestDB::new();
	let batch = SchemaBatch::new();
	batch.put::<TestSchema1>(&TestField(1), &TestField(11)).unwrap();

	db.write_durable_batch_sync(DurableWriteBatch::from_schema_batch(batch))
		.unwrap();

	assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), Some(TestField(11)));
}

#[test]
fn test_schema_put_get() {
	let db = TestDB::new();

	db.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
	db.put::<TestSchema1>(&TestField(1), &TestField(1)).unwrap();
	db.put::<TestSchema1>(&TestField(2), &TestField(2)).unwrap();
	db.put::<TestSchema2>(&TestField(2), &TestField(3)).unwrap();
	db.put::<TestSchema2>(&TestField(3), &TestField(4)).unwrap();
	db.put::<TestSchema2>(&TestField(4), &TestField(5)).unwrap();

	assert_eq!(db.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
	assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), Some(TestField(1)),);
	assert_eq!(db.get::<TestSchema1>(&TestField(2)).unwrap(), Some(TestField(2)),);
	assert_eq!(db.get::<TestSchema1>(&TestField(3)).unwrap(), None);

	assert_eq!(db.get::<TestSchema2>(&TestField(1)).unwrap(), None);
	assert_eq!(db.get::<TestSchema2>(&TestField(2)).unwrap(), Some(TestField(3)),);
	assert_eq!(db.get::<TestSchema2>(&TestField(3)).unwrap(), Some(TestField(4)),);
	assert_eq!(db.get::<TestSchema2>(&TestField(4)).unwrap(), Some(TestField(5)),);
}

#[test]
fn multi_get_preserves_input_order_and_missing_entries() {
	let db = TestDB::new();
	db.put::<TestSchema1>(&TestField(2), &TestField(20)).unwrap();

	let values = db
		.multi_get::<TestSchema1>(&[TestField(1), TestField(2), TestField(1)])
		.unwrap();

	assert_eq!(values, vec![None, Some(TestField(20)), None]);
}

#[test]
fn clear_schema_removes_only_target_column_family() {
	let db = TestDB::new();
	db.put::<TestSchema1>(&TestField(1), &TestField(10)).unwrap();
	db.put::<TestSchema2>(&TestField(1), &TestField(20)).unwrap();

	db.clear_schema::<TestSchema1>().unwrap();

	assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), None);
	assert_eq!(db.get::<TestSchema2>(&TestField(1)).unwrap(), Some(TestField(20)));
}

fn collect_values<S: Schema>(db: &TestDB) -> Vec<(S::Key, S::Value)> {
	let mut iter = db.iter::<S>().expect("Failed to create iterator.");
	iter.seek_to_first();
	iter.collect::<AppResult<Vec<_>>>().unwrap()
}

fn gen_expected_values(values: &[(u32, u32)]) -> Vec<(TestField, TestField)> {
	values
		.iter()
		.cloned()
		.map(|(x, y)| (TestField(x), TestField(y)))
		.collect()
}

#[test]
fn test_single_schema_batch() {
	let db = TestDB::new();

	let db_batch = SchemaBatch::new();
	db_batch.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
	db_batch.put::<TestSchema1>(&TestField(1), &TestField(1)).unwrap();
	db_batch.put::<TestSchema1>(&TestField(2), &TestField(2)).unwrap();
	db_batch.put::<TestSchema2>(&TestField(3), &TestField(3)).unwrap();
	db_batch.delete::<TestSchema2>(&TestField(4)).unwrap();
	db_batch.delete::<TestSchema2>(&TestField(3)).unwrap();
	db_batch.put::<TestSchema2>(&TestField(4), &TestField(4)).unwrap();
	db_batch.put::<TestSchema2>(&TestField(5), &TestField(5)).unwrap();

	db.write_schemas(db_batch).unwrap();

	assert_eq!(
		collect_values::<TestSchema1>(&db),
		gen_expected_values(&[(0, 0), (1, 1), (2, 2)]),
	);
	assert_eq!(
		collect_values::<TestSchema2>(&db),
		gen_expected_values(&[(4, 4), (5, 5)]),
	);
}

#[test]
fn test_schema_batch_splits_by_column_family_groups() {
	let db = TestDB::new();

	let db_batch = SchemaBatch::new();
	db_batch.put::<TestSchema1>(&TestField(1), &TestField(11)).unwrap();
	db_batch.put::<TestSchema2>(&TestField(2), &TestField(22)).unwrap();

	let mut split_batches =
		db_batch.into_column_family_batches(&[&[TestSchema1::COLUMN_FAMILY_NAME], &[TestSchema2::COLUMN_FAMILY_NAME]]);

	db.write_schemas(split_batches.remove(0)).unwrap();
	assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), Some(TestField(11)));
	assert_eq!(db.get::<TestSchema2>(&TestField(2)).unwrap(), None);

	db.write_schemas(split_batches.remove(0)).unwrap();
	assert_eq!(db.get::<TestSchema2>(&TestField(2)).unwrap(), Some(TestField(22)));
}

#[test]
fn test_two_schema_batches() {
	let db = TestDB::new();

	let db_batch1 = SchemaBatch::new();
	db_batch1.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
	db_batch1.put::<TestSchema1>(&TestField(1), &TestField(1)).unwrap();
	db_batch1.put::<TestSchema1>(&TestField(2), &TestField(2)).unwrap();
	db_batch1.delete::<TestSchema1>(&TestField(2)).unwrap();
	db.write_schemas(db_batch1).unwrap();

	assert_eq!(
		collect_values::<TestSchema1>(&db),
		gen_expected_values(&[(0, 0), (1, 1)]),
	);

	let db_batch2 = SchemaBatch::new();
	db_batch2.delete::<TestSchema2>(&TestField(3)).unwrap();
	db_batch2.put::<TestSchema2>(&TestField(3), &TestField(3)).unwrap();
	db_batch2.put::<TestSchema2>(&TestField(4), &TestField(4)).unwrap();
	db_batch2.put::<TestSchema2>(&TestField(5), &TestField(5)).unwrap();
	db.write_schemas(db_batch2).unwrap();

	assert_eq!(
		collect_values::<TestSchema1>(&db),
		gen_expected_values(&[(0, 0), (1, 1)]),
	);
	assert_eq!(
		collect_values::<TestSchema2>(&db),
		gen_expected_values(&[(3, 3), (4, 4), (5, 5)]),
	);
}

#[test]
fn test_reopen() {
	let tmpdir = aptos_temppath::TempPath::new();
	{
		let db = open_db(&tmpdir);
		db.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
		assert_eq!(db.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
	}
	{
		let db = open_db(&tmpdir);
		assert_eq!(db.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
	}
}

#[test]
fn test_open_read_only() {
	let tmpdir = aptos_temppath::TempPath::new();
	{
		let db = open_db(&tmpdir);
		db.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
	}
	{
		let db = open_db_read_only(&tmpdir);
		assert_eq!(db.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
		assert!(db.put::<TestSchema1>(&TestField(1), &TestField(1)).is_err());
	}
}

#[test]
fn test_open_as_secondary() {
	let tmpdir = aptos_temppath::TempPath::new();
	let tmpdir_sec = aptos_temppath::TempPath::new();

	let db = open_db(&tmpdir);
	db.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();

	let db_sec = open_db_as_secondary(&tmpdir, &tmpdir_sec);
	assert_eq!(db_sec.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
}

#[test]
fn test_report_size() {
	let db = TestDB::new();

	for i in 0..1000 {
		let db_batch = SchemaBatch::new();
		db_batch.put::<TestSchema1>(&TestField(i), &TestField(i)).unwrap();
		db_batch.put::<TestSchema2>(&TestField(i), &TestField(i)).unwrap();
		db.write_schemas(db_batch).unwrap();
	}

	db.flush_cf("TestCF1").unwrap();
	db.flush_cf("TestCF2").unwrap();

	assert!(db.get_property("TestCF1", "rocksdb.estimate-live-data-size").unwrap() > 0);
	assert!(db.get_property("TestCF2", "rocksdb.estimate-live-data-size").unwrap() > 0);
	assert_eq!(
		db.get_property("default", "rocksdb.estimate-live-data-size").unwrap(),
		0
	);
}

#[test]
fn test_checkpoint() {
	let tmpdir = aptos_temppath::TempPath::new();
	let checkpoint = aptos_temppath::TempPath::new();
	{
		let db = open_db(&tmpdir);
		db.put::<TestSchema1>(&TestField(0), &TestField(0)).unwrap();
		db.create_checkpoint(&checkpoint).unwrap();
	}
	{
		let db = open_db(&tmpdir);
		assert_eq!(db.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);

		let cp = open_db(&checkpoint);
		assert_eq!(cp.get::<TestSchema1>(&TestField(0)).unwrap(), Some(TestField(0)),);
		cp.put::<TestSchema1>(&TestField(1), &TestField(1)).unwrap();
		assert_eq!(cp.get::<TestSchema1>(&TestField(1)).unwrap(), Some(TestField(1)),);
		assert_eq!(db.get::<TestSchema1>(&TestField(1)).unwrap(), None);
	}
}

#[test]
fn test_unrecognised_column_family() {
	let tmpdir = aptos_temppath::TempPath::new();

	let mut opts = rocksdb::Options::default();
	opts.create_if_missing(true);
	opts.create_missing_column_families(true);

	let db = RksDB::open(tmpdir.path(), "test", vec!["cf1", "cf2"], &opts).unwrap();
	drop(db);

	RksDB::open(tmpdir.path(), "test", vec!["cf1"], &opts).unwrap();
}
