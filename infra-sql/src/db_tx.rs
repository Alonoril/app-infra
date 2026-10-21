use crate::{DatabaseConn, error::DBErr, sea_ext::page::PageQuery};
use infra_core::{map_err_logged, result::AppResult};
use sea_orm::{
	ConnectionTrait, DatabaseConnection, DatabaseTransaction, Paginator, SelectorTrait, TransactionTrait,
	prelude::async_trait,
};

/// sql database
pub struct DatabaseTx<'a, C: ConnectionTrait> {
	pub db_tx: &'a C,
}

impl<'a, C> DatabaseTx<'a, C>
where
	C: ConnectionTrait,
{
	pub fn new(db_tx: &'a C) -> Self {
		Self { db_tx }
	}
}

impl<'a, C> DatabaseTx<'a, C>
where
	C: ConnectionTrait,
{
	pub async fn fetch_page<'db, S>(
		&self,
		paginator: Paginator<'db, C, S>,
		page: PageQuery,
		biz: &str,
	) -> AppResult<(Vec<<S as SelectorTrait>::Item>, PageQuery)>
	where
		S: SelectorTrait + 'db,
	{
		let total = paginator
			.num_items()
			.await
			.map_err(map_err_logged!(DBErr::PaginatorItemsAndPages, biz))?;
		let page = page.with_total(total);

		// Fetch data for the specified page, page number starts from 0
		let items = paginator
			.fetch_page(page.page - 1)
			.await
			.map_err(map_err_logged!(DBErr::PaginatorFetchPage, biz))?;
		Ok((items, page))
	}
}

impl DatabaseConn {
	pub fn to_db_tx(&self) -> DatabaseTx<'_, DatabaseConnection> {
		DatabaseTx::new(&self.pool)
	}

	pub async fn begin_tx(&self, biz: &str) -> AppResult<DatabaseTransaction> {
		let tx = self
			.pool
			.begin()
			.await
			.map_err(map_err_logged!(DBErr::SqlxTxOpenError, biz))?;
		Ok(tx)
	}
}

#[async_trait::async_trait]
pub trait DbTxCommit {
	async fn commit_tx(mut self, biz: &str) -> AppResult<()>;
}

#[async_trait::async_trait]
impl DbTxCommit for DatabaseTransaction {
	async fn commit_tx(mut self, biz: &str) -> AppResult<()> {
		self.commit()
			.await
			.map_err(map_err_logged!(DBErr::SqlxTxCommitError, biz))
	}
}
