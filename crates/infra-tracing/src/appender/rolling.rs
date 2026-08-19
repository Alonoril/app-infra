//! Daily rolling appender and maintenance worker.

use super::{
	gzip::compress_log_file,
	retention::{daily_format, parse_log_date, prune_old_logs},
};
use std::{
	fs::{self, File, OpenOptions},
	io::{self, BufWriter, Write},
	path::{Path, PathBuf},
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
		mpsc::{self, Sender},
	},
	thread::{self, JoinHandle},
	time::Duration as StdDuration,
};
use time::{Date, OffsetDateTime, Time, format_description::FormatItem};

const FILE_BUFFER_CAPACITY: usize = 64 * 1024;
type DateSource = Box<dyn Fn() -> Date + Send + Sync>;

/// Daily rolling writer that compresses completed log files with gzip.
///
/// The active file is buffered and owned by the tracing writer thread.
/// Completed files are compressed and pruned by a dedicated background thread.
pub struct CompressingRollingFileAppender {
	writer: BufWriter<File>,
	maintenance: MaintenanceWorker,
	_rollover_scheduler: Option<RolloverScheduler>,
	rollover_requested: Arc<AtomicBool>,
	current_date: Date,
	date_source: DateSource,
	directory: PathBuf,
	filename_prefix: String,
	filename_suffix: String,
}

impl CompressingRollingFileAppender {
	/// Creates a daily appender that writes `prefix.YYYY-MM-DD.suffix` and
	/// compresses completed files to `prefix.YYYY-MM-DD.suffix.gz`.
	pub fn daily_gzip(
		directory: impl AsRef<Path>,
		filename_prefix: impl Into<String>,
		filename_suffix: impl Into<String>,
	) -> io::Result<Self> {
		Self::daily_gzip_with_max_log_files(directory, filename_prefix, filename_suffix, None)
	}

	/// Creates a daily gzip appender and keeps at most `max_log_files` matching
	/// `.log`/`.log.gz` files. Passing `None` disables retention pruning.
	pub fn daily_gzip_with_max_log_files(
		directory: impl AsRef<Path>,
		filename_prefix: impl Into<String>,
		filename_suffix: impl Into<String>,
		max_log_files: Option<usize>,
	) -> io::Result<Self> {
		let directory = directory.as_ref().to_path_buf();
		let (filename_prefix, filename_suffix) = (filename_prefix.into(), filename_suffix.into());
		let date_source: DateSource = Box::new(|| OffsetDateTime::now_utc().date());
		let current_date = date_source();
		let writer = open_log_file(&directory, &filename_prefix, &filename_suffix, current_date)?;
		let maintenance = MaintenanceWorker::spawn(directory, filename_prefix, filename_suffix, max_log_files);
		maintenance.scan(current_date);
		let rollover_requested = Arc::new(AtomicBool::new(false));
		let rollover_scheduler = RolloverScheduler::spawn(Arc::clone(&rollover_requested));

		Ok(Self::from_parts(
			writer,
			maintenance,
			Some(rollover_scheduler),
			rollover_requested,
			current_date,
			date_source,
		))
	}

	fn from_parts(
		writer: BufWriter<File>,
		maintenance: MaintenanceWorker,
		rollover_scheduler: Option<RolloverScheduler>,
		rollover_requested: Arc<AtomicBool>,
		current_date: Date,
		date_source: DateSource,
	) -> Self {
		Self {
			writer,
			directory: maintenance.directory.clone(),
			filename_prefix: maintenance.filename_prefix.clone(),
			filename_suffix: maintenance.filename_suffix.clone(),
			maintenance,
			_rollover_scheduler: rollover_scheduler,
			rollover_requested,
			current_date,
			date_source,
		}
	}

	#[cfg(test)]
	fn new_for_test(
		directory: impl AsRef<Path>,
		filename_prefix: impl Into<String>,
		filename_suffix: impl Into<String>,
		rollover_requested: Arc<AtomicBool>,
		date_source: impl Fn() -> Date + Send + Sync + 'static,
	) -> io::Result<Self> {
		let directory = directory.as_ref().to_path_buf();
		let filename_prefix = filename_prefix.into();
		let filename_suffix = filename_suffix.into();
		let date_source: DateSource = Box::new(date_source);
		let current_date = date_source();
		let writer = open_log_file(&directory, &filename_prefix, &filename_suffix, current_date)?;
		let maintenance = MaintenanceWorker::spawn(directory, filename_prefix, filename_suffix, None);
		Ok(Self::from_parts(
			writer,
			maintenance,
			None,
			rollover_requested,
			current_date,
			date_source,
		))
	}

	fn rollover_if_requested(&mut self) -> io::Result<()> {
		if !self.rollover_requested.load(Ordering::Relaxed) {
			return Ok(());
		}
		let current_date = (self.date_source)();
		if current_date == self.current_date {
			self.rollover_requested.store(false, Ordering::Relaxed);
			return Ok(());
		}

		self.writer.flush()?;
		let writer = open_log_file(
			&self.directory,
			&self.filename_prefix,
			&self.filename_suffix,
			current_date,
		)?;
		self.writer = writer;
		self.current_date = current_date;
		self.rollover_requested.store(false, Ordering::Relaxed);
		self.maintenance.scan(current_date);
		Ok(())
	}
}

impl Write for CompressingRollingFileAppender {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.rollover_if_requested()?;
		self.writer.write(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.writer.flush()
	}
}

fn open_log_file(
	directory: &Path,
	filename_prefix: &str,
	filename_suffix: &str,
	date: Date,
) -> io::Result<BufWriter<File>> {
	fs::create_dir_all(directory)?;
	let date = date.format(&daily_format()).map_err(io::Error::other)?;
	let path = directory.join(format!("{filename_prefix}.{date}.{filename_suffix}"));
	let file = OpenOptions::new().create(true).append(true).open(path)?;
	Ok(BufWriter::with_capacity(FILE_BUFFER_CAPACITY, file))
}

#[derive(Debug)]
struct RolloverScheduler {
	shutdown: Sender<()>,
	join_handle: Option<JoinHandle<()>>,
}

impl RolloverScheduler {
	fn spawn(rollover_requested: Arc<AtomicBool>) -> Self {
		let (shutdown, shutdown_rx) = mpsc::channel();
		let join_handle = thread::Builder::new()
			.name("log-rollover".to_owned())
			.spawn(move || {
				loop {
					let wait = duration_until_next_utc_midnight();
					match shutdown_rx.recv_timeout(wait) {
						Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
						Err(mpsc::RecvTimeoutError::Timeout) => rollover_requested.store(true, Ordering::Relaxed),
					}
				}
			})
			.expect("failed to spawn log rollover scheduler");
		Self {
			shutdown,
			join_handle: Some(join_handle),
		}
	}
}

impl Drop for RolloverScheduler {
	fn drop(&mut self) {
		let _ = self.shutdown.send(());
		if let Some(join_handle) = self.join_handle.take() {
			let _ = join_handle.join();
		}
	}
}

fn duration_until_next_utc_midnight() -> StdDuration {
	let now = OffsetDateTime::now_utc();
	let next_date = now.date().next_day().expect("UTC date should have a next day");
	let next_midnight = next_date.with_time(Time::MIDNIGHT).assume_utc();
	StdDuration::try_from(next_midnight - now).unwrap_or(StdDuration::from_secs(1))
}

#[derive(Debug)]
struct MaintenanceWorker {
	tx: Sender<MaintenanceCommand>,
	join_handle: Option<JoinHandle<()>>,
	directory: PathBuf,
	filename_prefix: String,
	filename_suffix: String,
}

impl MaintenanceWorker {
	fn spawn(
		directory: PathBuf,
		filename_prefix: String,
		filename_suffix: String,
		max_log_files: Option<usize>,
	) -> Self {
		let (tx, rx) = mpsc::channel();
		let worker_directory = directory.clone();
		let worker_filename_prefix = filename_prefix.clone();
		let worker_filename_suffix = filename_suffix.clone();
		let join_handle = thread::Builder::new()
			.name("log-maintenance".to_owned())
			.spawn(move || {
				let format = daily_format();
				while let Ok(command) = rx.recv() {
					match command {
						MaintenanceCommand::Scan { before_date } => {
							if let Err(err) = compress_completed_logs(
								&directory,
								&filename_prefix,
								&filename_suffix,
								before_date,
								&format,
							) {
								tracing::error!(reason = %err, "failed to gzip rolled log files");
								eprintln!("failed to gzip rolled log files: {err}");
							}
							if let Some(max_log_files) = max_log_files
								&& let Err(err) = prune_old_logs(
									&directory,
									&filename_prefix,
									&filename_suffix,
									max_log_files,
									&format,
								) {
								tracing::error!(reason = %err, "failed to prune old log files");
								eprintln!("failed to prune old log files: {err}");
							}
						}
						MaintenanceCommand::Shutdown => break,
					}
				}
			})
			.expect("failed to spawn log maintenance worker");

		Self {
			tx,
			join_handle: Some(join_handle),
			directory: worker_directory,
			filename_prefix: worker_filename_prefix,
			filename_suffix: worker_filename_suffix,
		}
	}

	fn scan(&self, before_date: Date) {
		let _ = self.tx.send(MaintenanceCommand::Scan { before_date });
	}
}

impl Drop for MaintenanceWorker {
	fn drop(&mut self) {
		let _ = self.tx.send(MaintenanceCommand::Shutdown);
		if let Some(join_handle) = self.join_handle.take()
			&& let Err(err) = join_handle.join()
		{
			eprintln!("log maintenance worker panicked: {err:?}");
		}
	}
}

#[derive(Debug)]
enum MaintenanceCommand {
	Scan { before_date: Date },
	Shutdown,
}

fn compress_completed_logs(
	directory: &Path,
	filename_prefix: &str,
	filename_suffix: &str,
	before_date: Date,
	format: &[FormatItem<'_>],
) -> io::Result<()> {
	let entries = match fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
		Err(err) => return Err(err),
	};

	for entry in entries {
		let entry = entry?;
		let path = entry.path();
		if !entry.file_type()?.is_file() {
			continue;
		}

		let filename = entry.file_name();
		let Some(filename) = filename.to_str() else {
			continue;
		};
		let Some(log_date) = parse_log_date(filename, filename_prefix, filename_suffix, format) else {
			continue;
		};
		if log_date < before_date {
			compress_log_file(&path)?;
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::{
		Arc, Mutex,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	};

	#[test]
	fn daily_gzip_writes_current_log_with_suffix() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let today = OffsetDateTime::now_utc()
			.date()
			.format(&daily_format())
			.expect("date should format");
		let expected_path = dir.path().join(format!("default.{today}.log"));
		let mut appender = CompressingRollingFileAppender::daily_gzip(dir.path(), "default", "log")
			.expect("appender should be created");

		appender.write_all(b"hello\n").expect("log should be written");
		appender.flush().expect("log should be flushed");

		assert_eq!(
			fs::read_to_string(expected_path).expect("current log should exist"),
			"hello\n"
		);
	}

	#[test]
	fn compresses_only_completed_logs() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let old_path = dir.path().join("default.2026-05-30.log");
		let current_path = dir.path().join("default.2026-05-31.log");
		fs::write(&old_path, b"old log\n").expect("old log should be written");
		fs::write(&current_path, b"current log\n").expect("current log should be written");

		compress_completed_logs(
			dir.path(),
			"default",
			"log",
			Date::from_calendar_date(2026, time::Month::May, 31).expect("date should be valid"),
			&daily_format(),
		)
		.expect("compression should succeed");

		assert!(!old_path.exists());
		assert!(dir.path().join("default.2026-05-30.log.gz").exists());
		assert!(current_path.exists());
	}

	#[test]
	fn normal_writes_do_not_read_current_date() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let rollover_requested = Arc::new(AtomicBool::new(false));
		let date_reads = Arc::new(AtomicUsize::new(0));
		let mut appender =
			CompressingRollingFileAppender::new_for_test(dir.path(), "default", "log", rollover_requested, {
				let date_reads = Arc::clone(&date_reads);
				move || {
					date_reads.fetch_add(1, Ordering::Relaxed);
					Date::from_calendar_date(2026, time::Month::June, 6).expect("date should be valid")
				}
			})
			.expect("appender should be created");
		date_reads.store(0, Ordering::Relaxed);

		appender.write_all(b"one\n").expect("first log should be written");
		appender.write_all(b"two\n").expect("second log should be written");

		assert_eq!(date_reads.load(Ordering::Relaxed), 0);
	}

	#[test]
	fn requested_rollover_reads_date_once_and_switches_file() {
		let dir = tempfile::tempdir().expect("tempdir should be created");
		let rollover_requested = Arc::new(AtomicBool::new(false));
		let date_reads = Arc::new(AtomicUsize::new(0));
		let dates = Arc::new(Mutex::new(vec![
			Date::from_calendar_date(2026, time::Month::June, 6).expect("date should be valid"),
			Date::from_calendar_date(2026, time::Month::June, 7).expect("date should be valid"),
		]));
		let mut appender = CompressingRollingFileAppender::new_for_test(
			dir.path(),
			"default",
			"log",
			Arc::clone(&rollover_requested),
			{
				let date_reads = Arc::clone(&date_reads);
				let dates = Arc::clone(&dates);
				move || {
					date_reads.fetch_add(1, Ordering::Relaxed);
					dates.lock().expect("dates lock should not be poisoned").remove(0)
				}
			},
		)
		.expect("appender should be created");
		date_reads.store(0, Ordering::Relaxed);
		rollover_requested.store(true, Ordering::Release);

		appender
			.write_all(b"new day\n")
			.expect("new day's log should be written");
		appender.flush().expect("log should be flushed");

		assert_eq!(date_reads.load(Ordering::Relaxed), 1);
		assert_eq!(
			fs::read_to_string(dir.path().join("default.2026-06-07.log")).expect("new log should exist"),
			"new day\n"
		);
	}
}
