//! Single-file gzip compression.

use flate2::{Compression, write::GzEncoder};
use std::{
	fs::{self, File},
	io::{self, BufReader, BufWriter},
	path::{Path, PathBuf},
};

const TMP_EXTENSION: &str = "gz.tmp";

pub(super) fn compress_log_file(path: &Path) -> io::Result<()> {
	let gzip_path = gzip_path_for(path);
	if gzip_path.exists() {
		return Ok(());
	}

	let tmp_path = gzip_path.with_extension(TMP_EXTENSION);
	let _ = fs::remove_file(&tmp_path);

	let input = File::open(path)?;
	let tmp_file = File::create(&tmp_path)?;
	let mut reader = BufReader::new(input);
	let writer = BufWriter::new(tmp_file);
	let mut encoder = GzEncoder::new(writer, Compression::default());
	io::copy(&mut reader, &mut encoder)?;
	let writer = encoder.finish()?;
	writer.into_inner()?.sync_all()?;

	fs::rename(&tmp_path, &gzip_path)?;
	fs::remove_file(path)?;
	Ok(())
}

fn gzip_path_for(path: &Path) -> PathBuf {
	path.with_extension(format!(
		"{}.gz",
		path.extension().and_then(|ext| ext.to_str()).unwrap_or_default()
	))
}

#[cfg(test)]
mod tests {
	use super::*;
	use flate2::read::GzDecoder;
	use std::io::Read;

	#[test]
	fn compressed_file_can_be_decoded() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let log_path = dir.path().join("default.2026-05-30.log");
		fs::write(&log_path, b"line one\nline two\n").expect("log should be written");

		compress_log_file(&log_path).expect("compression should succeed");

		let gzip_file = File::open(dir.path().join("default.2026-05-30.log.gz")).expect("gzip should exist");
		let mut decoder = GzDecoder::new(gzip_file);
		let mut decoded = String::new();
		decoder.read_to_string(&mut decoded).expect("gzip should decode");
		assert_eq!(decoded, "line one\nline two\n");
		assert!(!log_path.exists());
	}

	#[test]
	fn existing_gzip_file_is_not_overwritten() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let log_path = dir.path().join("default.2026-05-30.log");
		let gzip_path = dir.path().join("default.2026-05-30.log.gz");
		fs::write(&log_path, b"new log\n").expect("log should be written");
		fs::write(&gzip_path, b"existing gzip").expect("existing gzip should be written");

		compress_log_file(&log_path).expect("compression should be skipped");

		assert!(log_path.exists());
		assert_eq!(fs::read(&gzip_path).expect("gzip should be readable"), b"existing gzip");
	}
}
