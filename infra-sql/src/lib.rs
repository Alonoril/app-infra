use crate::error::DBErr;
use infra_core::{map_err_logged, result::AppResult};
use sea_orm::{ConnectOptions, Database as SeaDatabase, DatabaseConnection};
use std::{ops::Deref, time::Duration};
use tracing::info;

pub mod cfgs;
pub mod db_tx;
pub mod error;
pub mod macros;
pub mod sea_ext;
pub mod utils;

pub use async_trait::async_trait;

use crate::cfgs::DbCfgTrait;

#[async_trait::async_trait]
pub trait SqlxMigrateTrait {
	async fn migrate(&self, conn: &DatabaseConnection) -> AppResult<()>;
}

#[async_trait::async_trait]
pub trait DatabaseTrait<T, Cfg, Mgr>
where
	Cfg: DbCfgTrait + Sync + Send,
	Mgr: SqlxMigrateTrait + Sync + Send,
{
	async fn setup(cfg: &Cfg, migrate: &Mgr) -> AppResult<T>;

	async fn connect(cfg: &Cfg, _: &Mgr) -> AppResult<DatabaseConnection> {
		cfg.validate()?;
		let mut opt = ConnectOptions::new(cfg.db_url());
		opt.max_connections(cfg.max_conns())
			.min_connections(cfg.min_conns())
			.connect_timeout(Duration::from_secs(cfg.conn_timeout_secs()))
			.acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs()))
			.idle_timeout(Duration::from_secs(cfg.idle_timeout_secs()))
			.max_lifetime(Duration::from_secs(cfg.max_lifetime_secs()));

		let pool = SeaDatabase::connect(opt)
			.await
			.map_err(map_err_logged!(DBErr::InitDbPoolErr, cfg.debug_db_url()))?;

		info!(database_url = %cfg.debug_db_url(), "connected to database");
		Ok(pool)
	}
}

/// Database Connection
#[derive(Debug, Clone)]
pub struct DatabaseConn {
	pub pool: DatabaseConnection,
}

impl DatabaseConn {
	pub fn new(pool: DatabaseConnection) -> Self {
		Self { pool }
	}
}

#[async_trait::async_trait]
impl<Cfg, Mgr> DatabaseTrait<DatabaseConn, Cfg, Mgr> for DatabaseConn
where
	Cfg: DbCfgTrait,
	Mgr: SqlxMigrateTrait + Sync + Send,
{
	// let db = <Self as DatabaseTrait<DatabaseConn, DbCfg, Mg>>::connect(cfg).await?;
	async fn setup(cfg: &Cfg, migrate: &Mgr) -> AppResult<DatabaseConn> {
		let conn = Self::connect(cfg, migrate).await?;
		if cfg.run_migrations() {
			migrate.migrate(&conn).await?;
		}
		Ok(Self { pool: conn })
	}
}

impl Deref for DatabaseConn {
	type Target = DatabaseConnection;

	fn deref(&self) -> &Self::Target {
		&self.pool
	}
}

#[async_trait::async_trait]
pub trait ServerVersion {
	async fn version(&self) -> AppResult<String>;
}
