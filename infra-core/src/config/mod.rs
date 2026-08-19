mod bs_cfg;

pub use bs_cfg::{BaseConfig, RtEnv};

use crate::{
	map_err_logged,
	result::{AppResult, SysErr},
};
use figment::{
	Figment,
	providers::{Env, Format, Toml, Yaml},
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{path::PathBuf, sync::Arc};

pub trait GlobalConfigClient<C>
where
	C: DeserializeOwned + Send + Sync + Clone + 'static,
{
	fn get(&self) -> Arc<C>;

	fn cache(&mut self, config: C);
}

pub trait ConfigExt
where
	Self: for<'de> Deserialize<'de>,
{
	/// Load the configuration from the file at the value of the args(ENV/cli) `CONFIG`
	/// or `config.yaml` by default, with an overlay provided by environment variables prefixed with
	/// `"APP__"` and split/nested via `"__"`.
	fn load(path: PathBuf) -> AppResult<Self> {
		let config = Figment::new()
			.merge(Toml::string(""))
			.merge(Yaml::string(""))
			.merge(Yaml::file_exact(path))
			.merge(Env::prefixed("APP__").split("__"))
			.extract()
			.map_err(map_err_logged!(SysErr::ConfigLoadFailed))?;

		Ok(config)
	}
}

impl<T> ConfigExt for T where T: for<'de> Deserialize<'de> {}

#[cfg(test)]
mod tests {
	use super::ConfigExt;
	use crate::result::{ErrCodeTrait, SysErr};
	use serde::Deserialize;
	use std::{path::PathBuf, sync::Once};

	static INIT_TRACING: Once = Once::new();

	#[derive(Debug, Deserialize)]
	struct TestConfig {
		_name: String,
	}

	fn init_test_tracing() {
		INIT_TRACING.call_once(|| {
			tracing_subscriber::fmt()
				.with_max_level(tracing::Level::ERROR)
				.with_test_writer()
				.init();
		});
	}

	#[test]
	fn load_logs_source_error_when_config_file_is_missing() {
		init_test_tracing();

		let path = PathBuf::from(format!(
			"{}infra-core-missing-config-{}.yaml",
			std::env::temp_dir().display(),
			std::process::id()
		));

		let err = TestConfig::load(path).expect_err("missing config should fail");

		assert_eq!(err.domain(), SysErr::ConfigLoadFailed.domain());
		assert_eq!(err.code(), SysErr::ConfigLoadFailed.code());
		assert_eq!(err.message(), SysErr::ConfigLoadFailed.message());
	}
}
