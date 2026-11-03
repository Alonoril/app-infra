use alloy::{
	rpc::json_rpc::{RequestPacket, ResponsePacket},
	transports::{TransportError, TransportErrorKind},
};
use alloy_ext::transports::layers::{HealthCheckConfig, HealthChecker, ScoredTransport};
use std::{
	future::{Ready, ready},
	sync::{Arc, Mutex},
	task::{Context, Poll},
};
use tower_service::Service;
use tracing::{Event, Subscriber};
use tracing_subscriber::{Layer, layer::Context as LayerContext, prelude::*, registry::LookupSpan};

#[derive(Clone)]
struct ProbeTransport;

impl Service<RequestPacket> for ProbeTransport {
	type Response = ResponsePacket;
	type Error = TransportError;
	type Future = Ready<Result<ResponsePacket, TransportError>>;

	fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Ready(Ok(()))
	}

	fn call(&mut self, _: RequestPacket) -> Self::Future {
		tracing::warn!(target: "probe_trace_test", "probe failed");
		ready(Err(TransportErrorKind::custom_str("test failure")))
	}
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Vec<String>>>>);

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for Capture {
	fn on_event(&self, event: &Event<'_>, context: LayerContext<'_, S>) {
		if event.metadata().target() == "probe_trace_test" {
			let names = context
				.event_scope(event)
				.map(|scope| scope.from_root().map(|span| span.name().to_owned()).collect())
				.unwrap_or_default();
			self.0.lock().unwrap().push(names);
		}
	}
}

#[tokio::test]
async fn spawned_probes_keep_the_round_and_node_spans() {
	let capture = Capture::default();
	tracing::subscriber::set_global_default(tracing_subscriber::registry().with(capture.clone())).unwrap();
	let mut config = HealthCheckConfig::default();
	config.performance_benchmark.test_count = 1;
	let checker = HealthChecker::new(config);
	let transport = Arc::new(ScoredTransport::new(7, ProbeTransport));
	checker.register_node(7, &transport);

	let results = checker.run_health_check().await.unwrap();
	assert!(!results[0].is_healthy);
	let report = checker.run_performance_benchmark().await.unwrap();
	assert_eq!(report.overall_stats.successful_nodes, 0);
	assert_eq!(
		*capture.0.lock().unwrap(),
		vec![
			vec!["health_check", "rpc.health_probe"],
			vec!["performance_benchmark", "rpc.performance_probe"],
		]
	);
}
