//! Rolled log discovery and retention pruning.

use std::{
	fs, io,
	path::{Path, PathBuf},
};
use time::{
	Date,
	format_description::{self, FormatItem},
};

pub(super) fn prune_old_logs(
	directory: &Path,
	filename_prefix: &str,
	filename_suffix: &str,
	max_log_files: usize,
	format: &[FormatItem<'_>],
) -> io::Result<()> {
	if max_log_files == 0 {
		return Ok(());
	}

	let mut files = matching_log_files(directory, filename_prefix, filename_suffix, format)?;
	if files.len() <= max_log_files {
		return Ok(());
	}

	files.sort_by(|left, right| right.date.cmp(&left.date).then_with(|| right.path.cmp(&left.path)));

	for file in files.into_iter().skip(max_log_files) {
		fs::remove_file(file.path)?;
	}

	Ok(())
}

pub(super) fn parse_log_date(
	filename: &str,
	filename_prefix: &str,
	filename_suffix: &str,
	format: &[FormatItem<'_>],
) -> Option<Date> {
	let date = filename
		.strip_prefix(filename_prefix)?
		.strip_prefix('.')?
		.strip_suffix(filename_suffix)?
		.strip_suffix('.')?;
	Date::parse(date, format).ok()
}

pub(super) fn daily_format() -> Vec<FormatItem<'static>> {
	format_description::parse("[year]-[month]-[day]").expect("daily log date format must be valid")
}

fn matching_log_files(
	directory: &Path,
	filename_prefix: &str,
	filename_suffix: &str,
	format: &[FormatItem<'_>],
) -> io::Result<Vec<LogFile>> {
	let entries = match fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
		Err(err) => return Err(err),
	};
	let mut files = Vec::new();

	for entry in entries {
		let entry = entry?;
		if !entry.file_type()?.is_file() {
			continue;
		}

		let filename = entry.file_name();
		let Some(filename) = filename.to_str() else {
			continue;
		};
		let Some(date) = parse_log_or_gzip_date(filename, filename_prefix, filename_suffix, format) else {
			continue;
		};
		files.push(LogFile {
			path: entry.path(),
			date,
		});
	}

	Ok(files)
}

#[derive(Debug)]
struct LogFile {
	path: PathBuf,
	date: Date,
}

fn parse_log_or_gzip_date(
	filename: &str,
	filename_prefix: &str,
	filename_suffix: &str,
	format: &[FormatItem<'_>],
) -> Option<Date> {
	parse_log_date(filename, filename_prefix, filename_suffix, format).or_else(|| {
		let uncompressed_filename = filename.strip_suffix(".gz")?;
		parse_log_date(uncompressed_filename, filename_prefix, filename_suffix, format)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn prunes_old_log_and_gzip_files() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		for day in 1..=12 {
			let path = if day % 2 == 0 {
				dir.path().join(format!("default.2026-05-{day:02}.log.gz"))
			} else {
				dir.path().join(format!("default.2026-05-{day:02}.log"))
			};
			fs::write(path, format!("day {day}\n")).expect("sample log should be written");
		}

		prune_old_logs(dir.path(), "default", "log", 10, &daily_format()).expect("prune should succeed");

		assert!(!dir.path().join("default.2026-05-01.log").exists());
		assert!(!dir.path().join("default.2026-05-02.log.gz").exists());
		assert!(dir.path().join("default.2026-05-03.log").exists());
		assert!(dir.path().join("default.2026-05-12.log.gz").exists());
	}
}
