use infra_core::{
	define_app_error_codes,
	result::{AppError, ErrCodeTrait},
};
use std::borrow::Cow;

define_app_error_codes! {
	/// Stable RocksDB infrastructure error codes.
	RdbErr("RdbCore") {
		/// A requested item is not found.
		NotFound = (1001, "RocksDB item not found"),
		/// Requested too many items.
		TooManyRequested = (1002, "Too many RocksDB items requested"),
		/// A state root is missing, usually after pruning.
		MissingRoot = (1003, "Missing RocksDB state root"),
		/// A caller supplied invalid parameters.
		InvalidParams = (2001, "Invalid RocksDB parameters"),
		/// RocksDB returned an incomplete result.
		RocksDbIncompleteResult = (3001, "RocksDB result is incomplete"),
		/// RocksDB returned an error.
		RocksDb = (3002, "RocksDB operation failed"),
		/// IO failed at a RocksDB boundary.
		Io = (3003, "RocksDB IO operation failed"),
		/// Codec serialization or deserialization failed.
		Codec = (4001, "RocksDB codec failed"),
		/// Other non-classified error.
		Other = (9001, "Other RocksDB error"),
	}
}

/// Stable RocksDB error detail ids.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RdbDetail {
	/// No extra detail.
	None,
	/// Column family related detail.
	ColumnFamily,
	/// RocksDB property related detail.
	Property,
	/// TTL expiration timestamp detail.
	TtlExpireAt,
	/// Batch size detail.
	BatchSize,
	/// Anyhow source error detail.
	Anyhow,
	/// Parse integer source error detail.
	ParseInt,
	/// BCS codec detail.
	Bcs,
	/// Bincode encode detail.
	BincodeEncode,
	/// Bincode decode detail.
	BincodeDecode,
	/// Generic IO source error detail.
	Io,
	/// RocksDB NotFound kind.
	RocksDbNotFound,
	/// RocksDB Corruption kind.
	RocksDbCorruption,
	/// RocksDB NotSupported kind.
	RocksDbNotSupported,
	/// RocksDB InvalidArgument kind.
	RocksDbInvalidArgument,
	/// RocksDB IOError kind.
	RocksDbIo,
	/// RocksDB MergeInProgress kind.
	RocksDbMergeInProgress,
	/// RocksDB ShutdownInProgress kind.
	RocksDbShutdownInProgress,
	/// RocksDB TimedOut kind.
	RocksDbTimedOut,
	/// RocksDB Aborted kind.
	RocksDbAborted,
	/// RocksDB Busy kind.
	RocksDbBusy,
	/// RocksDB Expired kind.
	RocksDbExpired,
	/// RocksDB TryAgain kind.
	RocksDbTryAgain,
	/// RocksDB CompactionTooLarge kind.
	RocksDbCompactionTooLarge,
	/// RocksDB ColumnFamilyDropped kind.
	RocksDbColumnFamilyDropped,
	/// RocksDB Unknown kind.
	RocksDbUnknown,
	/// Unknown raw detail id.
	Raw(u32),
}

impl RdbDetail {
	pub const fn id(self) -> u32 {
		match self {
			Self::None => 0,
			Self::ColumnFamily => 1,
			Self::Property => 2,
			Self::TtlExpireAt => 3,
			Self::BatchSize => 4,
			Self::Anyhow => 10,
			Self::ParseInt => 11,
			Self::Bcs => 20,
			Self::BincodeEncode => 21,
			Self::BincodeDecode => 22,
			Self::Io => 30,
			Self::RocksDbNotFound => 100,
			Self::RocksDbCorruption => 101,
			Self::RocksDbNotSupported => 102,
			Self::RocksDbInvalidArgument => 103,
			Self::RocksDbIo => 104,
			Self::RocksDbMergeInProgress => 105,
			Self::RocksDbShutdownInProgress => 106,
			Self::RocksDbTimedOut => 107,
			Self::RocksDbAborted => 108,
			Self::RocksDbBusy => 109,
			Self::RocksDbExpired => 110,
			Self::RocksDbTryAgain => 111,
			Self::RocksDbCompactionTooLarge => 112,
			Self::RocksDbColumnFamilyDropped => 113,
			Self::RocksDbUnknown => 114,
			Self::Raw(id) => id,
		}
	}

	pub const fn from_id(id: u32) -> Self {
		match id {
			0 => Self::None,
			1 => Self::ColumnFamily,
			2 => Self::Property,
			3 => Self::TtlExpireAt,
			4 => Self::BatchSize,
			10 => Self::Anyhow,
			11 => Self::ParseInt,
			20 => Self::Bcs,
			21 => Self::BincodeEncode,
			22 => Self::BincodeDecode,
			30 => Self::Io,
			100 => Self::RocksDbNotFound,
			101 => Self::RocksDbCorruption,
			102 => Self::RocksDbNotSupported,
			103 => Self::RocksDbInvalidArgument,
			104 => Self::RocksDbIo,
			105 => Self::RocksDbMergeInProgress,
			106 => Self::RocksDbShutdownInProgress,
			107 => Self::RocksDbTimedOut,
			108 => Self::RocksDbAborted,
			109 => Self::RocksDbBusy,
			110 => Self::RocksDbExpired,
			111 => Self::RocksDbTryAgain,
			112 => Self::RocksDbCompactionTooLarge,
			113 => Self::RocksDbColumnFamilyDropped,
			114 => Self::RocksDbUnknown,
			raw => Self::Raw(raw),
		}
	}

	pub const fn detail(self) -> &'static str {
		match self {
			Self::None => "no detail",
			Self::ColumnFamily => "column family",
			Self::Property => "rocksdb property",
			Self::TtlExpireAt => "ttl expire_at",
			Self::BatchSize => "batch size",
			Self::Anyhow => "anyhow source error",
			Self::ParseInt => "parse int source error",
			Self::Bcs => "bcs codec",
			Self::BincodeEncode => "bincode encode",
			Self::BincodeDecode => "bincode decode",
			Self::Io => "io source error",
			Self::RocksDbNotFound => "rocksdb not found",
			Self::RocksDbCorruption => "rocksdb corruption",
			Self::RocksDbNotSupported => "rocksdb not supported",
			Self::RocksDbInvalidArgument => "rocksdb invalid argument",
			Self::RocksDbIo => "rocksdb io error",
			Self::RocksDbMergeInProgress => "rocksdb merge in progress",
			Self::RocksDbShutdownInProgress => "rocksdb shutdown in progress",
			Self::RocksDbTimedOut => "rocksdb timed out",
			Self::RocksDbAborted => "rocksdb aborted",
			Self::RocksDbBusy => "rocksdb busy",
			Self::RocksDbExpired => "rocksdb expired",
			Self::RocksDbTryAgain => "rocksdb try again",
			Self::RocksDbCompactionTooLarge => "rocksdb compaction too large",
			Self::RocksDbColumnFamilyDropped => "rocksdb column family dropped",
			Self::RocksDbUnknown => "rocksdb unknown error",
			Self::Raw(_) => "raw detail id",
		}
	}
}

#[inline(always)]
pub fn rdb_error(code: RdbErr, detail: RdbDetail) -> AppError {
	AppError::from_code_msg(code, rdb_detail_message(detail))
}

#[inline(always)]
pub fn rdb_error_code(code: RdbErr) -> AppError {
	AppError::from_code(code)
}

#[inline(always)]
pub fn not_found() -> AppError {
	rdb_error_code(RdbErr::NotFound)
}

#[inline(always)]
pub fn too_many_requested() -> AppError {
	rdb_error_code(RdbErr::TooManyRequested)
}

#[inline(always)]
pub fn missing_root() -> AppError {
	rdb_error_code(RdbErr::MissingRoot)
}

#[inline(always)]
pub fn invalid_params(detail: RdbDetail) -> AppError {
	rdb_error(RdbErr::InvalidParams, detail)
}

#[inline(always)]
pub fn column_family_missing() -> AppError {
	rdb_error(RdbErr::NotFound, RdbDetail::ColumnFamily)
}

#[inline(always)]
pub fn property_missing() -> AppError {
	rdb_error(RdbErr::NotFound, RdbDetail::Property)
}

#[inline(always)]
pub fn invalid_ttl_expire_at() -> AppError {
	rdb_error(RdbErr::InvalidParams, RdbDetail::TtlExpireAt)
}

#[inline(always)]
pub fn invalid_batch_size() -> AppError {
	rdb_error(RdbErr::InvalidParams, RdbDetail::BatchSize)
}

#[inline(always)]
pub fn codec_fault(detail: RdbDetail) -> AppError {
	rdb_error(RdbErr::Codec, detail)
}

#[inline(always)]
pub fn from_anyhow_error(error: anyhow::Error) -> AppError {
	rdb_error_with_source(RdbErr::Other, RdbDetail::Anyhow, error)
}

#[inline(always)]
pub fn from_rocksdb_error(error: rocksdb::Error) -> AppError {
	let detail = rocksdb_detail(error.kind());
	let code = if matches!(error.kind(), rocksdb::ErrorKind::Incomplete) {
		RdbErr::RocksDbIncompleteResult
	} else {
		RdbErr::RocksDb
	};

	rdb_error_with_source(code, detail, error)
}

#[inline(always)]
pub fn from_io_error(error: std::io::Error) -> AppError {
	rdb_error_with_source(RdbErr::Io, RdbDetail::Io, error)
}

#[inline(always)]
pub fn from_parse_int_error(error: std::num::ParseIntError) -> AppError {
	rdb_error_with_source(RdbErr::InvalidParams, RdbDetail::ParseInt, error)
}

#[inline(always)]
pub fn from_bcs_error(error: bcs::Error) -> AppError {
	rdb_error_with_source(RdbErr::Codec, RdbDetail::Bcs, error)
}

#[inline(always)]
pub fn from_bincode_encode_error(error: bincode::error::EncodeError) -> AppError {
	rdb_error_with_source(RdbErr::Codec, RdbDetail::BincodeEncode, error)
}

#[inline(always)]
pub fn from_bincode_decode_error(error: bincode::error::DecodeError) -> AppError {
	rdb_error_with_source(RdbErr::Codec, RdbDetail::BincodeDecode, error)
}

pub fn rdb_code_detail(code: RdbErr, detail: RdbDetail) -> String {
	format!("{}:{}", code.code(), detail.id())
}

pub fn rdb_fault_message(code: RdbErr, detail: RdbDetail) -> Cow<'static, str> {
	rdb_message(code, detail)
}

#[inline]
fn rdb_error_with_source<E>(code: RdbErr, detail: RdbDetail, error: E) -> AppError
where
	E: std::fmt::Display,
{
	let detail_message = rdb_detail_message(detail);
	AppError::from_code_msg(code, format!("{detail_message}: {error}"))
}

#[inline]
fn rocksdb_detail(kind: rocksdb::ErrorKind) -> RdbDetail {
	match kind {
		rocksdb::ErrorKind::Incomplete => RdbDetail::RocksDbUnknown,
		rocksdb::ErrorKind::NotFound => RdbDetail::RocksDbNotFound,
		rocksdb::ErrorKind::Corruption => RdbDetail::RocksDbCorruption,
		rocksdb::ErrorKind::NotSupported => RdbDetail::RocksDbNotSupported,
		rocksdb::ErrorKind::InvalidArgument => RdbDetail::RocksDbInvalidArgument,
		rocksdb::ErrorKind::IOError => RdbDetail::RocksDbIo,
		rocksdb::ErrorKind::MergeInProgress => RdbDetail::RocksDbMergeInProgress,
		rocksdb::ErrorKind::ShutdownInProgress => RdbDetail::RocksDbShutdownInProgress,
		rocksdb::ErrorKind::TimedOut => RdbDetail::RocksDbTimedOut,
		rocksdb::ErrorKind::Aborted => RdbDetail::RocksDbAborted,
		rocksdb::ErrorKind::Busy => RdbDetail::RocksDbBusy,
		rocksdb::ErrorKind::Expired => RdbDetail::RocksDbExpired,
		rocksdb::ErrorKind::TryAgain => RdbDetail::RocksDbTryAgain,
		rocksdb::ErrorKind::CompactionTooLarge => RdbDetail::RocksDbCompactionTooLarge,
		rocksdb::ErrorKind::ColumnFamilyDropped => RdbDetail::RocksDbColumnFamilyDropped,
		rocksdb::ErrorKind::Unknown => RdbDetail::RocksDbUnknown,
	}
}

fn rdb_detail_message(detail: RdbDetail) -> &'static str {
	detail.detail()
}

fn rdb_message(code: RdbErr, detail: RdbDetail) -> Cow<'static, str> {
	let code_message = code.message();
	let detail_message = rdb_detail_message(detail);
	if detail == RdbDetail::None || detail_message.is_empty() {
		Cow::Borrowed(code_message)
	} else {
		let mut message = String::with_capacity(code_message.len() + 2 + detail_message.len());
		message.push_str(code_message);
		message.push_str(": ");
		message.push_str(detail_message);
		Cow::Owned(message)
	}
}

#[cfg(test)]
mod tests {
	use super::{RdbDetail, RdbErr, rdb_code_detail, rdb_error, rdb_fault_message};
	use infra_core::result::{AppError, ErrCodeTrait};

	#[test]
	fn rksdb_error_code_uses_rdb_domain() {
		let err = AppError::from_code(RdbErr::InvalidParams);

		assert_eq!(err.domain(), "RdbCore");
		assert_eq!(err.code(), RdbErr::InvalidParams.code());
		assert_eq!(err.message(), "Invalid RocksDB parameters");
	}

	#[test]
	fn rksdb_error_converts_to_app_error_with_detail_message() {
		let app = rdb_error(RdbErr::InvalidParams, RdbDetail::BatchSize);

		assert_eq!(app.domain(), "RdbCore");
		assert_eq!(app.code(), RdbErr::InvalidParams.code());
		assert_eq!(app.message(), "Invalid RocksDB parameters: batch size");
	}

	#[test]
	fn rksdb_error_formats_code_detail_and_message() {
		assert_eq!(rdb_code_detail(RdbErr::RocksDb, RdbDetail::RocksDbIo), "3002:104");
		assert_eq!(
			rdb_fault_message(RdbErr::RocksDb, RdbDetail::RocksDbIo),
			"RocksDB operation failed: rocksdb io error"
		);
	}
}
