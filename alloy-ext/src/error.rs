infra_core::define_app_error_codes! {
	AyeErr("SFB") {
		FailoverConfigInvalid = (1, "Serial fallback configuration is invalid"),
		FailoverAlreadyRunning = (2, "Serial fallback health checker is already running"),
		FailoverNotRunning = (3, "Serial fallback health checker is not running"),
		FailoverNoTransports = (4, "No transports available in serial fallback"),
		FailoverHealthCheckUnavailable = (5, "Health checker not available"),
		FailoverWarmupFailed = (6, "Failed to warmup serial fallback transports"),
		FailoverSerializationFailed = (7, "Failed to serialize serial fallback data"),
		FailoverFileOperationFailed = (8, "Serial fallback file operation failed"),
		FailoverBenchmarkFailed = (9, "Performance benchmark failed"),
		FailoverTaskJoinFailed = (10, "Serial fallback task join failed"),
	}
}
