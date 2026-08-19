use infra_tracing::appender::CompressingRollingFileAppender;
use std::{
	fs,
	io::{self, Write},
};

fn main() -> io::Result<()> {
	let log_dir = std::env::temp_dir().join(format!("infra-tracing-retention-demo-{}", std::process::id()));
	let _ = fs::remove_dir_all(&log_dir);
	fs::create_dir_all(&log_dir)?;

	for day in 1..=12 {
		let path = if day % 2 == 0 {
			log_dir.join(format!("default.2026-05-{day:02}.log.gz"))
		} else {
			log_dir.join(format!("default.2026-05-{day:02}.log"))
		};
		println!("creating {}", path.display());
		fs::write(path, format!("old sample log day {day}\n"))?;
	}

	let mut appender =
		CompressingRollingFileAppender::daily_gzip_with_max_log_files(&log_dir, "default", "log", Some(10))?;
	appender.write_all(b"retention demo log line\n")?;
	appender.flush()?;
	drop(appender);

	let mut files = fs::read_dir(&log_dir)?
		.map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
		.collect::<Result<Vec<_>, _>>()?;
	files.sort();

	println!("log dir: {}", log_dir.display());
	for file in files {
		println!("{file}");
	}

	Ok(())
}
