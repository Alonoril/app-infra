use crate::{DatabaseConn, ServerVersion, error::DBErr};
use infra_core::{map_err_logged, ok_or_logged, result::AppResult};
use sea_orm::{ConnectionTrait, DbBackend, Statement};

#[async_trait::async_trait]
impl ServerVersion for DatabaseConn {
	async fn version(&self) -> AppResult<String> {
		let stmt = Statement::from_string(DbBackend::Postgres, "SELECT version()");
		let row = self
			.pool
			.query_one(stmt)
			.await
			.map_err(map_err_logged!(DBErr::GetVersion))?;
		let row = row.ok_or_else(ok_or_logged!(DBErr::VersionNotFound))?;
		row.try_get("", "version")
			.map_err(map_err_logged!(DBErr::TryGetVersion))
	}
}
