use crate::config::{BaseConfig, RtEnv};
pub use clap::Parser;
use std::path::PathBuf;
use tracing::Level;

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
enum AppEnv {
	Development,
	Test,
	Production,
}

#[derive(clap::Parser)]
pub struct AppCliArgs {
	#[clap(long, env, value_enum)]
	app_env: AppEnv,
	/// log level
	#[clap(long, env, default_value = "INFO")]
	#[arg(value_parser = parse_level)]
	log_level: Option<Level>,
	/// Path to application configuration file (or template for local test mode).
	#[clap(long, env, value_parser)]
	config: Option<PathBuf>,
	/// Git commit hash
	#[clap(long, short = 'c', value_parser)]
	commit: bool,
}

impl AppCliArgs {
	pub fn commit(&self) -> bool {
		self.commit
	}
}

fn parse_level(level: &str) -> anyhow::Result<Level> {
	let level: Level = level
		.parse()
		.map_err(|e| anyhow::anyhow!("Invalid log level: {:?}", e))?;
	Ok(level)
}

impl From<AppCliArgs> for BaseConfig {
	fn from(value: AppCliArgs) -> Self {
		// eprintln!("AppCliArgs#app_env: {:?}", value.app_env);
		let env: RtEnv = match value.app_env {
			AppEnv::Development => RtEnv::Development,
			AppEnv::Test => RtEnv::Test,
			AppEnv::Production => RtEnv::Production,
		};

		Self {
			rt_env: env,
			log_level: value.log_level,
			config_path: value.config,
		}
	}
}
