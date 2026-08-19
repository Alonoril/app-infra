use crate::{Logger, LoggerGuard};
use infra_core::config::BaseConfig;
use tracing::Level;

pub fn setup_logger() -> anyhow::Result<LoggerGuard> {
	let mut bs_cfg = BaseConfig::default();
	// bs_cfg.log_level = Some(Level::INFO);
	let logger = Logger::default();

	// init logger
	let guard = logger.init(&bs_cfg);
	Ok(guard)
}
