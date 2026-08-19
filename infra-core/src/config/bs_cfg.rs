use crate::{
	app_err,
	result::{AppResult, SysErr},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::Level;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogOutput {
	Console,
	File,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RtEnv {
	#[default]
	Development,
	Test,
	Production,
}

impl RtEnv {
	pub fn log_target(&self) -> LogOutput {
		match self {
			RtEnv::Development => LogOutput::Console,
			RtEnv::Test => LogOutput::File,
			RtEnv::Production => LogOutput::File,
		}
	}
}

#[derive(Clone, Debug)]
pub struct BaseConfig {
	pub rt_env: RtEnv,
	/// log level
	pub log_level: Option<Level>,
	pub config_path: Option<PathBuf>,
}

impl BaseConfig {
	pub fn new(rt_env: RtEnv) -> Self {
		Self {
			rt_env,
			log_level: Some(Level::INFO),
			..Default::default()
		}
	}

	pub fn with_config_path(self, path: PathBuf) -> Self {
		Self {
			config_path: Some(path),
			..self
		}
	}

	pub fn log_level(&self) -> Level {
		self.log_level.unwrap_or(Level::INFO)
	}

	pub fn config_path(&self) -> AppResult<PathBuf> {
		self.config_path.clone().ok_or(app_err!(SysErr::NoCfgFile))
	}
}

impl Default for BaseConfig {
	fn default() -> Self {
		Self {
			rt_env: RtEnv::Development,
			log_level: Some(Level::DEBUG),
			config_path: Some(PathBuf::from("./configs/app-config.yaml")),
		}
	}
}
