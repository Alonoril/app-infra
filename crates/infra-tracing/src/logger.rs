//! Initialize logger.

use crate::appender::CompressingRollingFileAppender;
use infra_core::config::{BaseConfig, RtEnv};
use serde::Deserialize;
use std::{panic, path::PathBuf, thread};
use tracing::{error, level_filters::LevelFilter};
use tracing_appender::non_blocking::{ErrorCounter, NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::{
	EnvFilter, Registry,
	fmt::{self, Layer},
	layer::SubscriberExt,
	registry,
	util::SubscriberInitExt,
};

const LOG_BUFFERED_LINES_LIMIT: usize = 8_192;

/// Keeps the background log writer alive and reports lines dropped under load.
#[derive(Debug)]
pub struct LoggerGuard {
	_worker_guard: WorkerGuard,
	error_counter: ErrorCounter,
}

impl LoggerGuard {
	pub fn dropped_lines(&self) -> usize {
		self.error_counter.dropped_lines()
	}
}

/// Initialize logger (tracing and panic hook).
#[derive(Debug, Clone, Deserialize)]
pub struct Logger {
	pub path: PathBuf,
	#[serde(default = "default_file_name")]
	pub file_name: String,
	pub directives: Vec<String>,
	#[serde(default = "default_max_log_files")]
	pub max_log_files: usize,
}

impl Default for Logger {
	fn default() -> Self {
		Self {
			path: PathBuf::new(),
			file_name: default_file_name(),
			directives: Vec::new(),
			max_log_files: default_max_log_files(),
		}
	}
}

impl Logger {
	pub fn with_path(self, path: PathBuf) -> Self {
		Self { path, ..self }
	}

	pub fn init(&self, app_args: &BaseConfig) -> LoggerGuard {
		let (non_blocking, guard) = match app_args.rt_env {
			RtEnv::Development => non_blocking(std::io::stdout()),
			_ => {
				let dir = self.path.join("logs");
				let file_logger = CompressingRollingFileAppender::daily_gzip_with_max_log_files(
					dir,
					self.file_name.as_str(),
					"log",
					Some(self.max_log_files),
				)
				.expect("initializing rolling gzip file appender failed");
				non_blocking(file_logger)
			}
		};
		let error_counter = non_blocking.error_counter();
		let filter = self.build_env_filter(app_args);

		match app_args.rt_env {
			RtEnv::Development => registry().with(console_layer(non_blocking)).with(filter).init(),
			_ => registry().with(file_layer(non_blocking)).with(filter).init(),
		}
		self.panic_hook();

		LoggerGuard {
			_worker_guard: guard,
			error_counter,
		}
	}

	fn build_env_filter(&self, app_args: &BaseConfig) -> EnvFilter {
		let app_env: RtEnv = app_args.rt_env;
		let max_level = match app_args.log_level {
			Some(level) => level.into(),
			None => match app_env {
				RtEnv::Development => LevelFilter::DEBUG,
				_ => LevelFilter::INFO,
			},
		};

		let mut env_filter =
			EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(max_level.to_string()));
		for directive in &self.directives {
			env_filter = env_filter.add_directive(directive.parse().expect("invalid directive"));
		}

		env_filter
	}

	fn panic_hook(&self) {
		// catch panic and log them using tracing instead of default output to StdErr
		panic::set_hook(Box::new(|info| {
			let thread = thread::current();
			let thread = thread.name().unwrap_or("unknown");

			let msg = match info.payload().downcast_ref::<&'static str>() {
				Some(s) => *s,
				None => match info.payload().downcast_ref::<String>() {
					Some(s) => &**s,
					None => "Box<Any>",
				},
			};

			// let backtrace = backtrace::Backtrace::new();

			match info.location() {
				Some(location) => {
					// without backtrace
					if msg.starts_with("notrace - ") {
						error!(
							target: "panic", "thread '{}' panicked at '{}': {}:{}",
							thread,
							msg.strip_prefix("notrace - ").unwrap_or(msg),
							location.file(),
							location.line()
						);
					}
					// with backtrace
					else {
						error!(
							target: "panic", "thread '{}' panicked at '{}': {}:{}",
							thread,
							msg,
							location.file(),
							location.line(),
							// backtrace
						);
					}
				}
				None => {
					// without backtrace
					if msg.starts_with("notrace - ") {
						error!(
							target: "panic", "thread '{}' panicked at '{}'",
							thread,
							msg.strip_prefix("notrace - ").unwrap_or(msg),
						);
					}
					// with backtrace
					else {
						error!(
							target: "panic", "thread '{}' panicked at '{}'",
							thread,
							msg,
							// backtrace
						);
					}
				}
			}
		}));
	}
}

fn non_blocking(writer: impl std::io::Write + Send + 'static) -> (NonBlocking, WorkerGuard) {
	NonBlockingBuilder::default()
		.buffered_lines_limit(LOG_BUFFERED_LINES_LIMIT)
		.lossy(true)
		.thread_name("log-writer")
		.finish(writer)
}

fn console_layer<W>(writer: W) -> impl tracing_subscriber::Layer<Registry>
where
	W: for<'writer> fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
	Layer::new()
		.with_line_number(true)
		.with_thread_names(true)
		.with_thread_ids(true)
		.with_ansi(true)
		.with_writer(writer)
}

fn file_layer<W>(writer: W) -> impl tracing_subscriber::Layer<Registry>
where
	W: for<'writer> fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
	#[cfg(feature = "log-json")]
	{
		Layer::new()
			.json()
			.with_file(true)
			.with_line_number(true)
			.with_thread_names(false)
			.with_thread_ids(false)
			.with_current_span(true)
			.with_span_list(false)
			.with_writer(writer)
	}

	#[cfg(not(feature = "log-json"))]
	{
		Layer::new()
			.with_file(true)
			.with_line_number(true)
			.with_thread_names(false)
			.with_thread_ids(false)
			.with_writer(writer)
	}
}

fn default_max_log_files() -> usize {
	30
}

fn default_file_name() -> String {
	String::from("default")
}

#[cfg(test)]
mod tests {
	use super::Logger;
	use figment::{
		Figment,
		providers::{Format, Toml},
	};
	use std::{
		io::{self, Write},
		sync::{
			Arc, Mutex,
			mpsc::{Receiver, SyncSender, sync_channel},
		},
	};
	use tracing_appender::non_blocking::NonBlockingBuilder;
	use tracing_subscriber::{layer::SubscriberExt, registry};

	#[derive(Clone, Default)]
	struct SharedWriter(Arc<Mutex<Vec<u8>>>);

	impl SharedWriter {
		fn contents(&self) -> String {
			String::from_utf8(self.0.lock().expect("writer lock should not be poisoned").clone())
				.expect("logs should be UTF-8")
		}
	}

	impl Write for SharedWriter {
		fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
			self.0
				.lock()
				.expect("writer lock should not be poisoned")
				.extend_from_slice(buf);
			Ok(buf.len())
		}

		fn flush(&mut self) -> io::Result<()> {
			Ok(())
		}
	}

	struct BlockingWriter {
		started: SyncSender<()>,
		release: Receiver<()>,
	}

	impl Write for BlockingWriter {
		fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
			self.started.send(()).expect("test should wait for writer");
			self.release.recv().expect("test should release writer");
			Ok(buf.len())
		}

		fn flush(&mut self) -> io::Result<()> {
			Ok(())
		}
	}

	#[test]
	fn file_layer_writes_diagnostic_json() {
		let writer = SharedWriter::default();
		let make_writer = {
			let writer = writer.clone();
			move || writer.clone()
		};
		let subscriber = registry().with(super::file_layer(make_writer));

		tracing::subscriber::with_default(subscriber, || {
			let span = tracing::info_span!("request", request_id = 42);
			let _entered = span.enter();
			tracing::info!(answer = 7, "indexed");
		});

		let event: serde_json::Value = serde_json::from_str(writer.contents().trim()).expect("file log should be JSON");
		assert_eq!(event["fields"]["answer"], 7);
		assert!(event["filename"].is_string());
		assert!(event["line_number"].is_number());
		assert_eq!(event["span"]["request_id"], 42);
		assert!(event.get("threadName").is_none());
		assert!(event.get("threadId").is_none());
		assert!(event.get("spans").is_none());
	}

	#[test]
	fn console_layer_keeps_text_format() {
		let writer = SharedWriter::default();
		let make_writer = {
			let writer = writer.clone();
			move || writer.clone()
		};
		let subscriber = registry().with(super::console_layer(make_writer));

		tracing::subscriber::with_default(subscriber, || tracing::info!("ready"));

		let output = writer.contents();
		assert!(output.contains("ready"));
		assert!(serde_json::from_str::<serde_json::Value>(output.trim()).is_err());
	}

	#[test]
	fn logger_guard_reports_dropped_lines() {
		let (started_tx, started_rx) = sync_channel(0);
		let (release_tx, release_rx) = sync_channel(0);
		let (mut writer, worker_guard) = NonBlockingBuilder::default()
			.buffered_lines_limit(1)
			.lossy(true)
			.finish(BlockingWriter {
				started: started_tx,
				release: release_rx,
			});
		let guard = super::LoggerGuard {
			error_counter: writer.error_counter(),
			_worker_guard: worker_guard,
		};

		writer.write_all(b"first\n").expect("first line should be accepted");
		started_rx.recv().expect("writer should start first line");
		writer
			.write_all(b"second\n")
			.expect("second line should fill the queue");
		writer
			.write_all(b"third\n")
			.expect("lossy writer should accept dropped line");

		assert_eq!(guard.dropped_lines(), 1);
		release_tx.send(()).expect("writer should be released");
		started_rx.recv().expect("writer should start second line");
		release_tx.send(()).expect("writer should finish second line");
	}

	#[test]
	fn file_name_defaults_to_default_when_missing() {
		let logger: Logger = Figment::new()
			.merge(Toml::string(
				r#"
				path = "/tmp"
				directives = []
				max_log_files = 10
				"#,
			))
			.extract()
			.expect("logger config should deserialize");

		assert_eq!(logger.file_name, "default");
	}

	#[test]
	fn file_name_uses_configured_value() {
		let logger: Logger = Figment::new()
			.merge(Toml::string(
				r#"
				path = "/tmp"
				file_name = "indexer"
				directives = []
				max_log_files = 10
				"#,
			))
			.extract()
			.expect("logger config should deserialize");

		assert_eq!(logger.file_name, "indexer");
	}
}
