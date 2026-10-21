infra_core::define_app_error_codes! {
	/// Stable error codes returned by the shared SQL infrastructure.
	DBErr("DBP") {
		InitDbPoolErr = (1, "error while initializing the database connection pool"),
		RunMigrationsErr = (2, "error while running database migrations"),
		SqlxTxOpenError = (3, "database transaction open error"),
		SqlxTxCommitError = (4, "database transaction commit error"),
		SqlxError = (5, "database error"),
		PaginatorItemsAndPages = (6, "get total items and pages error"),
		PaginatorFetchPage = (7, "execute paginator fetch_page error"),
		GetVersion = (8, "get server version error"),
		VersionNotFound = (9, "server version was not returned"),
		TryGetVersion = (10, "read server version from query result error"),
	}
}
