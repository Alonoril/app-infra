use std::{
	collections::HashMap,
	sync::{Arc, Weak},
	time::{Duration, Instant},
};

use alloy::primitives::Address;
use alloy::rpc::json_rpc::{Id, Request, RequestPacket, ResponsePacket, SerializedRequest};
use alloy::transports::{TransportError, TransportResult};
use infra_core::{result::AppResult, rt_pool::Tokio};
use parking_lot::RwLock;
use tokio::time::{interval, timeout};
use tower::Service;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::error::AyeErr;
use crate::transports::layers::{HealthCheckConfig, PerformanceBenchmarkConfig, ProbeMethod, ScoredTransport};
use infra_core::{err, map_err};

/// 健康检查器状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckerState {
	/// 是否正在运行
	pub running: bool,
	/// 最后检查时间（Unix时间戳毫秒）
	pub last_check_ms: Option<u64>,
	/// 检查统计信息
	pub stats: HealthCheckStats,
}

/// 健康检查统计信息
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckStats {
	/// 总检查次数
	pub total_checks: u64,
	/// 成功检查次数
	pub successful_checks: u64,
	/// 失败检查次数
	pub failed_checks: u64,
	/// 平均检查耗时（毫秒）
	pub avg_check_duration_ms: f64,
}

/// 单个节点的健康检查结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckResult {
	/// 节点ID
	pub node_id: usize,
	/// 是否健康
	pub is_healthy: bool,
	/// 延迟时间（毫秒）
	#[serde(with = "option_duration_ms")]
	pub latency: Option<Duration>,
	/// 错误信息
	pub error: Option<String>,
	/// 检查时间
	#[serde(with = "instant_as_millis")]
	pub timestamp: Instant,
}

/// 性能基准测试结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkResult {
	/// 节点ID
	pub node_id: usize,
	/// 是否成功
	pub success: bool,
	/// 延迟统计
	pub latency_stats: LatencyStats,
	/// 成功率
	pub success_rate: f64,
	/// 总测试次数
	pub total_tests: u32,
	/// 成功测试次数
	pub successful_tests: u32,
	/// 错误信息列表
	pub errors: Vec<String>,
	/// 测试时间戳
	#[serde(with = "instant_as_millis")]
	pub timestamp: Instant,
}

/// 延迟统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LatencyStats {
	/// 最小延迟（毫秒）
	pub min_ms: f64,
	/// 最大延迟（毫秒）
	pub max_ms: f64,
	/// 平均延迟（毫秒）
	pub avg_ms: f64,
	/// 中位数延迟（毫秒）
	pub median_ms: f64,
	/// P95延迟（毫秒）
	pub p95_ms: f64,
	/// P99延迟（毫秒）
	pub p99_ms: f64,
}

impl LatencyStats {
	/// 从延迟向量计算统计信息
	pub fn from_latencies(latencies: &[Duration]) -> Self {
		if latencies.is_empty() {
			return Self {
				min_ms: 0.0,
				max_ms: 0.0,
				avg_ms: 0.0,
				median_ms: 0.0,
				p95_ms: 0.0,
				p99_ms: 0.0,
			};
		}

		let mut latencies_ms: Vec<f64> = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
		latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

		let min_ms = latencies_ms[0];
		let max_ms = latencies_ms[latencies_ms.len() - 1];
		let avg_ms = latencies_ms.iter().sum::<f64>() / latencies_ms.len() as f64;

		let median_ms = if latencies_ms.len().is_multiple_of(2) {
			let mid = latencies_ms.len() / 2;
			(latencies_ms[mid - 1] + latencies_ms[mid]) / 2.0
		} else {
			latencies_ms[latencies_ms.len() / 2]
		};

		let p95_index = ((latencies_ms.len() as f64) * 0.95) as usize;
		let p95_ms = latencies_ms
			.get(p95_index.min(latencies_ms.len() - 1))
			.copied()
			.unwrap_or(max_ms);

		let p99_index = ((latencies_ms.len() as f64) * 0.99) as usize;
		let p99_ms = latencies_ms
			.get(p99_index.min(latencies_ms.len() - 1))
			.copied()
			.unwrap_or(max_ms);

		Self {
			min_ms,
			max_ms,
			avg_ms,
			median_ms,
			p95_ms,
			p99_ms,
		}
	}
}

/// 完整的基准测试报告
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkReport {
	/// 报告时间戳
	#[serde(with = "instant_as_millis")]
	pub timestamp: Instant,
	/// 总测试时长
	#[serde(with = "duration_as_millis")]
	pub total_duration: Duration,
	/// 节点基准测试结果
	pub node_results: Vec<BenchmarkResult>,
	/// 整体统计
	pub overall_stats: BenchmarkOverallStats,
}

/// 基准测试整体统计
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BenchmarkOverallStats {
	/// 测试的节点总数
	pub total_nodes: usize,
	/// 成功的节点数
	pub successful_nodes: usize,
	/// 整体成功率
	pub overall_success_rate: f64,
	/// 最佳节点ID
	pub best_node_id: Option<usize>,
	/// 最差节点ID
	pub worst_node_id: Option<usize>,
	/// 平均延迟（毫秒）
	pub avg_latency_ms: f64,
}

mod option_duration_ms {
	use serde::{Deserialize, Deserializer, Serializer};
	use std::time::Duration;

	pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		match duration {
			Some(d) => serializer.serialize_some(&d.as_millis()),
			None => serializer.serialize_none(),
		}
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
	where
		D: Deserializer<'de>,
	{
		let opt: Option<u128> = Option::deserialize(deserializer)?;
		Ok(opt.map(|millis| Duration::from_millis(millis as u64)))
	}
}

mod instant_as_millis {
	use serde::{Deserialize, Deserializer, Serializer};
	use std::time::{Instant, SystemTime, UNIX_EPOCH};

	pub fn serialize<S>(_instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		// 简化实现：使用当前时间戳
		let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
		serializer.serialize_u128(now.as_millis())
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
	where
		D: Deserializer<'de>,
	{
		let _millis: u128 = Deserialize::deserialize(deserializer)?;
		// 简化实现：返回当前时间
		Ok(Instant::now())
	}
}

mod duration_as_millis {
	use serde::{Deserialize, Deserializer, Serializer};
	use std::time::Duration;

	pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_u128(duration.as_millis())
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
	where
		D: Deserializer<'de>,
	{
		let millis: u128 = Deserialize::deserialize(deserializer)?;
		Ok(Duration::from_millis(millis as u64))
	}
}

/// 健康检查器
///
/// 后台定时任务，用于主动监控每个节点的健康状态
#[derive(Debug)]
pub struct HealthChecker<S> {
	/// 要检查的节点列表（弱引用避免循环引用）
	nodes: Arc<RwLock<HashMap<usize, Weak<ScoredTransport<S>>>>>,
	/// 健康检查配置
	config: HealthCheckConfig,
	/// 运行状态
	state: Arc<RwLock<HealthCheckerState>>,
	/// 停止信号
	stop_signal: tokio::sync::broadcast::Sender<()>,
	/// 性能基准测试器
	benchmark_stop_signal: tokio::sync::broadcast::Sender<()>,
}

impl<S> HealthChecker<S>
where
	S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError> + Clone + Send + Sync + 'static,
	S::Future: Send + 'static,
{
	/// 创建新的健康检查器
	pub fn new(config: HealthCheckConfig) -> Self {
		let (stop_signal, _) = tokio::sync::broadcast::channel(1);
		let (benchmark_stop_signal, _) = tokio::sync::broadcast::channel(1);

		Self {
			nodes: Arc::new(RwLock::new(HashMap::new())),
			config,
			state: Arc::new(RwLock::new(HealthCheckerState {
				running: false,
				last_check_ms: None,
				stats: HealthCheckStats::default(),
			})),
			stop_signal,
			benchmark_stop_signal,
		}
	}

	/// 启动健康检查器
	pub async fn start(&self) -> AppResult<()> {
		if !self.config.enabled {
			info!(target: "serial_failover:health", "Health checker is disabled in configuration");
			return Ok(());
		}

		// 检查是否已经在运行
		{
			let mut state = self.state.write();
			if state.running {
				return err!(AyeErr::FailoverAlreadyRunning);
			}
			state.running = true;
		}

		info!(target: "serial_failover:health",
			"Starting health checker with config: interval={}s, timeout={}s, recovery_threshold={}, failure_threshold={}",
			self.config.interval_secs,
			self.config.timeout_secs,
			self.config.recovery_threshold,
			self.config.failure_threshold
		);

		// 如果启用了启动检查，先执行一次
		if self.config.startup_check
			&& let Err(e) = self.run_health_check().await
		{
			warn!(target: "serial_failover:health", "Startup health check failed: {}", e);
		}

		// 启动健康检查定时任务
		let health_checker = self.clone();
		let mut stop_receiver = self.stop_signal.subscribe();

		Tokio.spawn(async move {
			let mut interval = interval(health_checker.config.interval());
			interval.tick().await; // 跳过第一次立即触发

			loop {
				tokio::select! {
					_ = interval.tick() => {
						if let Err(e) = health_checker.run_health_check().await {
							error!(target: "serial_failover:health", "Health check failed: {}", e);
						}
					}
					_ = stop_receiver.recv() => {
						info!(target: "serial_failover:health", "Health checker stopping...");
						break;
					}
				}
			}

			// 标记为停止状态
			{
				let mut state = health_checker.state.write();
				state.running = false;
			}

			info!(target: "serial_failover:health", "Health checker stopped");
		});

		// 启动性能基准测试器（如果启用）
		if self.config.performance_benchmark.enabled {
			self.start_performance_benchmark().await?;
		}

		Ok(())
	}

	/// 启动性能基准测试器
	pub async fn start_performance_benchmark(&self) -> AppResult<()> {
		let benchmark_config = &self.config.performance_benchmark;

		info!(target: "serial_failover:health",
			"Starting performance benchmark: interval={}s, test_count={}, timeout={}s",
			benchmark_config.interval_secs, benchmark_config.test_count, benchmark_config.timeout_secs
		);

		// 如果启用了启动测试，先执行一次
		if benchmark_config.startup_test
			&& let Err(e) = self.run_performance_benchmark().await
		{
			warn!(target: "serial_failover:health", "Startup performance benchmark failed: {}", e);
		}

		// 启动性能基准测试定时任务
		let health_checker = self.clone();
		let mut benchmark_stop_receiver = self.benchmark_stop_signal.subscribe();

		Tokio.spawn(async move {
			let mut interval = interval(health_checker.config.performance_benchmark.interval());
			interval.tick().await; // 跳过第一次立即触发

			loop {
				tokio::select! {
					_ = interval.tick() => {
						if let Err(e) = health_checker.run_performance_benchmark().await {
							error!(target: "serial_failover:health", "Performance benchmark failed: {}", e);
						}
					}
					_ = benchmark_stop_receiver.recv() => {
						info!(target: "serial_failover:health", "Performance benchmark stopping...");
						break;
					}
				}
			}

			info!(target: "serial_failover:health", "Performance benchmark stopped");
		});

		Ok(())
	}

	/// 停止健康检查器
	pub async fn stop(&self) -> AppResult<()> {
		if !self.state.read().running {
			return Ok(());
		}

		info!(target: "serial_failover:health", "Stopping health checker...");

		// 发送停止信号
		self.stop_signal
			.send(())
			.map_err(map_err!(AyeErr::FailoverNotRunning))?;

		// 停止性能基准测试器
		if self.config.performance_benchmark.enabled {
			self.stop_performance_benchmark().await?;
		}

		// 等待一段时间确保任务停止
		tokio::time::sleep(Duration::from_millis(100)).await;

		Ok(())
	}

	/// 停止性能基准测试器
	pub async fn stop_performance_benchmark(&self) -> AppResult<()> {
		info!(target: "serial_failover:health", "Stopping performance benchmark...");

		// 发送停止信号
		self.benchmark_stop_signal
			.send(())
			.map_err(map_err!(AyeErr::FailoverNotRunning))?;

		// 等待一段时间确保任务停止
		tokio::time::sleep(Duration::from_millis(100)).await;

		Ok(())
	}

	/// 手动执行一次健康检查
	#[instrument(skip(self), name = "health_check")]
	pub async fn run_health_check(&self) -> AppResult<Vec<HealthCheckResult>> {
		let start_time = Instant::now();
		let mut results = Vec::new();

		// 获取所有活跃的节点
		let nodes = self.filter_active_nodes();
		if nodes.is_empty() {
			if self.config.enable_logging {
				trace!(target: "serial_failover:health", "No nodes to check");
			}
			return Ok(results);
		}

		if self.config.enable_logging {
			debug!(target: "serial_failover:health", "Starting health check for {} nodes", nodes.len());
		}

		// 并发检查所有节点
		let mut check_tasks = Vec::new();
		for (node_id, transport) in nodes {
			let config = self.config.clone();
			let task = Tokio.spawn(async move { Self::check_single_node(node_id, transport, config).await });
			check_tasks.push(task);
		}

		// 等待所有检查完成
		for task in check_tasks {
			match task.await {
				Ok(result) => {
					results.push(result);
				}
				Err(e) => {
					return err!(
						AyeErr::FailoverTaskJoinFailed,
						format!("Health check task failed: {}", e)
					);
				}
			}
		}

		// 更新统计信息
		let total_duration = start_time.elapsed();
		self.update_stats(&results, total_duration);

		if self.config.enable_logging {
			let healthy_count = results.iter().filter(|r| r.is_healthy).count();
			debug!(target: "serial_failover:health",
				"Health check completed: {}/{} nodes healthy, took {:?}",
				healthy_count,
				results.len(),
				total_duration
			);
		}

		Ok(results)
	}

	/// 检查单个节点
	async fn check_single_node(
		node_id: usize,
		transport: Arc<ScoredTransport<S>>,
		config: HealthCheckConfig,
	) -> HealthCheckResult {
		let start_time = Instant::now();

		// 创建探测请求
		let request = Self::create_probe_request(&config.probe_method);

		// 执行检查
		let result = timeout(config.timeout(), Self::perform_probe(&transport, request)).await;

		let (is_healthy, latency, error) = match result {
			Ok(Ok(_)) => {
				let latency = start_time.elapsed();
				transport.track_health_success(latency);
				(true, Some(latency), None)
			}
			Ok(Err(e)) => {
				transport.track_health_failure();
				(false, None, Some(e.to_string()))
			}
			Err(_) => {
				transport.track_health_failure();
				(false, None, Some("Health check timeout".to_string()))
			}
		};

		HealthCheckResult {
			node_id,
			is_healthy,
			latency,
			error,
			timestamp: Instant::now(),
		}
	}

	/// 执行探测
	async fn perform_probe(
		transport: &Arc<ScoredTransport<S>>,
		request: RequestPacket,
	) -> TransportResult<ResponsePacket> {
		let mut transport_clone = transport.transport.clone();
		transport_clone.call(request).await
	}

	fn filter_active_nodes(&self) -> Vec<(usize, Arc<ScoredTransport<S>>)> {
		let nodes_guard = self.nodes.read();
		nodes_guard
			.iter()
			.filter_map(|(id, weak_ref)| weak_ref.upgrade().map(|transport| (*id, transport)))
			.collect::<Vec<_>>()
	}

	/// 手动执行一次性能基准测试
	#[instrument(skip(self), name = "performance_benchmark")]
	pub async fn run_performance_benchmark(&self) -> AppResult<BenchmarkReport> {
		let start_time = Instant::now();
		let benchmark_config = &self.config.performance_benchmark;

		// 获取所有活跃的节点
		let nodes = self.filter_active_nodes();
		if nodes.is_empty() {
			if benchmark_config.enable_logging {
				trace!(target: "serial_failover:health", "No nodes to benchmark");
			}
			return Ok(BenchmarkReport {
				timestamp: Instant::now(),
				total_duration: Duration::from_secs(0),
				node_results: vec![],
				overall_stats: BenchmarkOverallStats {
					total_nodes: 0,
					successful_nodes: 0,
					overall_success_rate: 0.0,
					best_node_id: None,
					worst_node_id: None,
					avg_latency_ms: 0.0,
				},
			});
		}

		if benchmark_config.enable_logging {
			debug!(target: "serial_failover:health",
				"🚀 Starting performance benchmark: {} nodes | {} tests per node | timeout={}s | concurrency={}",
				nodes.len(),
				benchmark_config.test_count,
				benchmark_config.timeout_secs,
				benchmark_config.max_concurrent_nodes
			);
		}

		// 分批并发基准测试（限制并发数）
		let mut all_results = Vec::new();
		let chunk_size = benchmark_config.max_concurrent_nodes;

		// 收集所有节点到向量中以避免生命周期问题
		let nodes_vec: Vec<(usize, Arc<ScoredTransport<S>>)> = nodes.into_iter().collect();

		for chunk in nodes_vec.chunks(chunk_size) {
			let mut chunk_tasks = Vec::new();

			for (node_id, transport) in chunk {
				let config = benchmark_config.clone();
				let transport = transport.clone();
				let node_id = *node_id;
				let task = Tokio.spawn(async move { Self::benchmark_single_node(node_id, transport, config).await });
				chunk_tasks.push(task);
			}

			// 等待当前批次完成
			for task in chunk_tasks {
				match task.await {
					Ok(result) => {
						all_results.push(result);
					}
					Err(e) => {
						return err!(
							AyeErr::FailoverBenchmarkFailed,
							format!("Performance benchmark task failed: {}", e)
						);
					}
				}
			}
		}

		let total_duration = start_time.elapsed();

		// 计算整体统计
		let overall_stats = Self::calculate_overall_stats(&all_results);

		// 如果启用自动重新排序，更新节点分数
		if benchmark_config.auto_reorder_after_test {
			self.apply_benchmark_results(&all_results).await;
		}

		if benchmark_config.enable_logging {
			let successful_count = all_results.iter().filter(|r| r.success).count();

			// 计算延迟统计
			let successful_results: Vec<&BenchmarkResult> = all_results.iter().filter(|r| r.success).collect();
			let avg_latency_ms = if !successful_results.is_empty() {
				successful_results.iter().map(|r| r.latency_stats.avg_ms).sum::<f64>() / successful_results.len() as f64
			} else {
				0.0
			};

			debug!(target: "serial_failover:health",
				"🏁 Performance benchmark completed: {}/{} nodes successful | duration: {:?} | avg latency: {:.1}ms | best node: {}",
				successful_count,
				all_results.len(),
				total_duration,
				avg_latency_ms,
				overall_stats
					.best_node_id
					.map(|id| format!("Node[{}]", id))
					.unwrap_or_else(|| "None".to_string())
			);
		}

		Ok(BenchmarkReport {
			timestamp: Instant::now(),
			total_duration,
			node_results: all_results,
			overall_stats,
		})
	}

	/// 对单个节点进行基准测试
	async fn benchmark_single_node(
		node_id: usize,
		transport: Arc<ScoredTransport<S>>,
		config: PerformanceBenchmarkConfig,
	) -> BenchmarkResult {
		let mut latencies = Vec::new();
		let mut errors = Vec::new();
		let mut successful_tests = 0;

		for i in 0..config.test_count {
			let start_time = Instant::now();

			// 创建基准测试请求
			let request = Self::create_probe_request(&config.test_method);

			// 执行测试
			let result = timeout(config.timeout(), Self::perform_probe(&transport, request)).await;

			match result {
				Ok(Ok(_)) => {
					let latency = start_time.elapsed();
					latencies.push(latency);
					successful_tests += 1;

					// 记录到transport的主动数据中
					transport.track_health_success(latency);
				}
				Ok(Err(e)) => {
					errors.push(format!("Test {}: {}", i + 1, e));
					transport.track_health_failure();
				}
				Err(_) => {
					errors.push(format!("Test {}: timeout", i + 1));
					transport.track_health_failure();
				}
			}

			// 在测试之间添加小的间隔以避免过载
			if i < config.test_count - 1 {
				tokio::time::sleep(Duration::from_millis(10)).await;
			}
		}

		let success_rate = successful_tests as f64 / config.test_count as f64;
		let latency_stats = LatencyStats::from_latencies(&latencies);

		BenchmarkResult {
			node_id,
			success: successful_tests > 0,
			latency_stats,
			success_rate,
			total_tests: config.test_count,
			successful_tests,
			errors,
			timestamp: Instant::now(),
		}
	}

	/// 计算整体统计信息
	fn calculate_overall_stats(results: &[BenchmarkResult]) -> BenchmarkOverallStats {
		if results.is_empty() {
			return BenchmarkOverallStats {
				total_nodes: 0,
				successful_nodes: 0,
				overall_success_rate: 0.0,
				best_node_id: None,
				worst_node_id: None,
				avg_latency_ms: 0.0,
			};
		}

		let total_nodes = results.len();
		let successful_nodes = results.iter().filter(|r| r.success).count();
		let overall_success_rate = successful_nodes as f64 / total_nodes as f64;

		// 找到最佳和最差节点（基于平均延迟）
		let mut best_node_id = None;
		let mut worst_node_id = None;
		let mut best_latency = f64::MAX;
		let mut worst_latency = 0.0;

		let mut total_latency = 0.0;
		let mut valid_latency_count = 0;

		for result in results.iter().filter(|r| r.success) {
			let avg_latency = result.latency_stats.avg_ms;

			if avg_latency < best_latency {
				best_latency = avg_latency;
				best_node_id = Some(result.node_id);
			}

			if avg_latency > worst_latency {
				worst_latency = avg_latency;
				worst_node_id = Some(result.node_id);
			}

			total_latency += avg_latency;
			valid_latency_count += 1;
		}

		let avg_latency_ms = if valid_latency_count > 0 {
			total_latency / valid_latency_count as f64
		} else {
			0.0
		};

		BenchmarkOverallStats {
			total_nodes,
			successful_nodes,
			overall_success_rate,
			best_node_id,
			worst_node_id,
			avg_latency_ms,
		}
	}

	/// 应用基准测试结果到节点分数
	async fn apply_benchmark_results(&self, results: &[BenchmarkResult]) {
		// 获取所有节点并应用基准测试结果
		let nodes_guard = self.nodes.read();
		let mut score_changes = Vec::new();

		for result in results {
			if let Some(weak_ref) = nodes_guard.get(&result.node_id)
				&& let Some(transport) = weak_ref.upgrade()
			{
				let old_score = transport.score();

				// 基于基准测试结果更新分数
				// 这里的逻辑已经在 benchmark_single_node 中通过 track_health_success/failure 实现
				// 分数会在 TransportMetrics 中自动重新计算

				let new_score = transport.score();
				let score_change = ((new_score - old_score) / old_score.max(0.001)) * 100.0;
				score_changes.push((result.node_id, old_score, new_score, score_change));
			}
		}

		// 记录分数变化的汇总信息
		if !score_changes.is_empty() {
			let successful_nodes = results.iter().filter(|r| r.success).count();
			let avg_score_change: f64 =
				score_changes.iter().map(|(_, _, _, change)| change).sum::<f64>() / score_changes.len() as f64;

			debug!(target: "serial_failover:health",
				"🎯 Performance benchmark scores updated: {}/{} nodes tested successfully | avg score change: {:+.2}%",
				successful_nodes,
				results.len(),
				avg_score_change
			);

			// 记录分数变化最大的前3个节点
			score_changes.sort_by(|a, b| b.3.abs().partial_cmp(&a.3.abs()).unwrap_or(std::cmp::Ordering::Equal));
			for (node_id, old_score, new_score, change) in score_changes.iter().take(3) {
				debug!(target: "serial_failover:health",
					"  📊 Node[{}] score change: {:.4} -> {:.4} ({:+.2}%)",
					node_id, old_score, new_score, change
				);
			}
		}
	}
}
impl<S> HealthChecker<S> {
	// 创建新的健康检查器（无约束版本用于测试）
	// pub fn new_unconstrained(config: HealthCheckConfig) -> Self {
	// 	let (stop_signal, _) = tokio::sync::broadcast::channel(1);
	// 	let (benchmark_stop_signal, _) = tokio::sync::broadcast::channel(1);
	//
	// 	Self {
	// 		nodes: Arc::new(RwLock::new(HashMap::new())),
	// 		config,
	// 		state: Arc::new(RwLock::new(HealthCheckerState {
	// 			running: false,
	// 			last_check_ms: None,
	// 			stats: HealthCheckStats::default(),
	// 		})),
	// 		stop_signal,
	// 		benchmark_stop_signal,
	// 	}
	// }

	/// 注册节点进行健康检查
	pub fn register_node(&self, node_id: usize, transport: &Arc<ScoredTransport<S>>) {
		let weak_ref = Arc::downgrade(transport);
		self.nodes.write().insert(node_id, weak_ref);

		if self.config.enable_logging {
			debug!(target: "serial_failover:health", "Registered node {} for health checks", node_id);
		}
	}

	/// 取消注册节点
	pub fn unregister_node(&self, node_id: usize) {
		self.nodes.write().remove(&node_id);

		if self.config.enable_logging {
			debug!(target: "serial_failover:health", "Unregistered node {} from health checks", node_id);
		}
	}

	/// 获取当前状态
	pub fn get_state(&self) -> HealthCheckerState {
		self.state.read().clone()
	}

	/// 创建探测请求
	fn create_probe_request(probe_method: &ProbeMethod) -> RequestPacket {
		let id = Id::Number(1);

		match probe_method {
			ProbeMethod::BlockNumber => {
				let req = Request::new("eth_blockNumber", id, ());
				let serialized = SerializedRequest::try_from(req).unwrap();
				RequestPacket::Single(serialized)
			}
			ProbeMethod::GetBalance { address } => {
				let addr = address.parse::<Address>().unwrap_or(Address::ZERO);
				let req = Request::new("eth_getBalance", id, (addr, "latest"));
				let serialized = SerializedRequest::try_from(req).unwrap();
				RequestPacket::Single(serialized)
			}
			ProbeMethod::NetVersion => {
				let req = Request::new("net_version", id, ());
				let serialized = SerializedRequest::try_from(req).unwrap();
				RequestPacket::Single(serialized)
			}
			ProbeMethod::ClientVersion => {
				let req = Request::new("web3_clientVersion", id, ());
				let serialized = SerializedRequest::try_from(req).unwrap();
				RequestPacket::Single(serialized)
			}
			ProbeMethod::Custom { method, params } => {
				let method_str = method.clone();
				let req = Request::new(method_str, id, params.clone());
				let serialized = SerializedRequest::try_from(req).unwrap();
				RequestPacket::Single(serialized)
			}
		}
	}

	/// 更新统计信息
	fn update_stats(&self, results: &[HealthCheckResult], duration: Duration) {
		let mut state = self.state.write();

		state.last_check_ms = Some(
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap_or_default()
				.as_millis() as u64,
		);
		state.stats.total_checks += 1;

		let successful = results.iter().filter(|r| r.is_healthy).count() as u64;
		let failed = results.len() as u64 - successful;

		state.stats.successful_checks += successful;
		state.stats.failed_checks += failed;

		// 更新平均检查耗时
		let total_checks = state.stats.total_checks;
		let current_avg = state.stats.avg_check_duration_ms;
		let new_duration_ms = duration.as_millis() as f64;

		state.stats.avg_check_duration_ms =
			(current_avg * (total_checks - 1) as f64 + new_duration_ms) / total_checks as f64;
	}

	/// 启动健康检查器（无约束版本用于测试）
	pub async fn start_unconstrained(&self) -> AppResult<()> {
		if !self.config.enabled {
			info!(target: "serial_failover:health", "Health checker is disabled in configuration");
			return Ok(());
		}

		// 检查是否已经在运行
		{
			let mut state = self.state.write();
			if state.running {
				return err!(AyeErr::FailoverAlreadyRunning);
			}
			state.running = true;
		}

		info!(target: "serial_failover:health",
			"Starting health checker: interval={}s, timeout={}s, recovery_threshold={}, failure_threshold={}",
			self.config.interval_secs,
			self.config.timeout_secs,
			self.config.recovery_threshold,
			self.config.failure_threshold
		);

		Ok(())
	}
}

impl<S> Clone for HealthChecker<S> {
	fn clone(&self) -> Self {
		Self {
			nodes: Arc::clone(&self.nodes),
			config: self.config.clone(),
			state: Arc::clone(&self.state),
			stop_signal: self.stop_signal.clone(),
			benchmark_stop_signal: self.benchmark_stop_signal.clone(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use alloy::providers::mock::MockTransport;
	use serde_json::json;

	#[tokio::test]
	async fn test_health_checker_creation() {
		let config = HealthCheckConfig::default();
		let checker = HealthChecker::<()>::new(config);

		let state = checker.get_state();
		assert!(!state.running);
		assert!(state.last_check_ms.is_none());
		assert_eq!(state.stats.total_checks, 0);
	}

	#[tokio::test]
	async fn test_health_checker_register_unregister() {
		let config = HealthCheckConfig::default();
		let checker = HealthChecker::<()>::new(config);

		let transport = Arc::new(ScoredTransport::new(1, ()));

		// 注册节点
		checker.register_node(1, &transport);
		assert_eq!(checker.nodes.read().len(), 1);

		// 取消注册节点
		checker.unregister_node(1);
		assert_eq!(checker.nodes.read().len(), 0);
	}

	#[tokio::test]
	async fn test_health_checker_disabled() {
		let mut config = HealthCheckConfig::default();
		config.enabled = false;

		let checker = HealthChecker::<()>::new(config);

		// 启动应该立即返回成功
		assert!(checker.start_unconstrained().await.is_ok());

		// 状态应该还是未运行
		let state = checker.get_state();
		assert!(!state.running);
	}

	#[tokio::test]
	async fn test_probe_method_requests() {
		let methods = vec![
			ProbeMethod::BlockNumber,
			ProbeMethod::GetBalance {
				address: "0x0000000000000000000000000000000000000000".to_string(),
			},
			ProbeMethod::NetVersion,
			ProbeMethod::ClientVersion,
			ProbeMethod::Custom {
				method: "custom_method".to_string(),
				params: json!({"param": "value"}),
			},
		];

		for method in methods {
			let request = HealthChecker::<()>::create_probe_request(&method);
			match request {
				RequestPacket::Single(req) => match method {
					ProbeMethod::BlockNumber => assert_eq!(req.method(), "eth_blockNumber"),
					ProbeMethod::GetBalance { .. } => assert_eq!(req.method(), "eth_getBalance"),
					ProbeMethod::NetVersion => assert_eq!(req.method(), "net_version"),
					ProbeMethod::ClientVersion => assert_eq!(req.method(), "web3_clientVersion"),
					ProbeMethod::Custom { method, .. } => assert_eq!(req.method(), &method),
				},
				_ => panic!("Expected single request"),
			}
		}
	}

	#[tokio::test]
	async fn test_health_check_result() {
		let result = HealthCheckResult {
			node_id: 1,
			is_healthy: true,
			latency: Some(Duration::from_millis(100)),
			error: None,
			timestamp: Instant::now(),
		};

		assert_eq!(result.node_id, 1);
		assert!(result.is_healthy);
		assert!(result.latency.is_some());
		assert!(result.error.is_none());
	}

	#[tokio::test]
	async fn test_health_check_stats() {
		let mut stats = HealthCheckStats::default();

		stats.total_checks = 10;
		stats.successful_checks = 8;
		stats.failed_checks = 2;
		stats.avg_check_duration_ms = 150.0;

		assert_eq!(stats.total_checks, 10);
		assert_eq!(stats.successful_checks, 8);
		assert_eq!(stats.failed_checks, 2);
		assert_eq!(stats.avg_check_duration_ms, 150.0);
	}

	#[test]
	fn test_latency_stats_calculation() {
		let latencies = vec![
			Duration::from_millis(50),
			Duration::from_millis(100),
			Duration::from_millis(150),
			Duration::from_millis(200),
			Duration::from_millis(250),
		];

		let stats = LatencyStats::from_latencies(&latencies);

		assert_eq!(stats.min_ms, 50.0);
		assert_eq!(stats.max_ms, 250.0);
		assert_eq!(stats.avg_ms, 150.0);
		assert_eq!(stats.median_ms, 150.0);
		assert!(stats.p95_ms >= 200.0);
		assert!(stats.p99_ms >= 200.0);
	}

	#[test]
	fn test_latency_stats_empty() {
		let latencies: Vec<Duration> = vec![];
		let stats = LatencyStats::from_latencies(&latencies);

		assert_eq!(stats.min_ms, 0.0);
		assert_eq!(stats.max_ms, 0.0);
		assert_eq!(stats.avg_ms, 0.0);
		assert_eq!(stats.median_ms, 0.0);
		assert_eq!(stats.p95_ms, 0.0);
		assert_eq!(stats.p99_ms, 0.0);
	}

	#[test]
	fn test_benchmark_result_creation() {
		let result = BenchmarkResult {
			node_id: 1,
			success: true,
			latency_stats: LatencyStats {
				min_ms: 50.0,
				max_ms: 200.0,
				avg_ms: 125.0,
				median_ms: 120.0,
				p95_ms: 180.0,
				p99_ms: 195.0,
			},
			success_rate: 0.8,
			total_tests: 5,
			successful_tests: 4,
			errors: vec!["Test 5: timeout".to_string()],
			timestamp: Instant::now(),
		};

		assert_eq!(result.node_id, 1);
		assert!(result.success);
		assert_eq!(result.success_rate, 0.8);
		assert_eq!(result.total_tests, 5);
		assert_eq!(result.successful_tests, 4);
		assert_eq!(result.errors.len(), 1);
	}

	#[test]
	fn test_benchmark_overall_stats_empty() {
		let results: Vec<BenchmarkResult> = vec![];
		let stats = HealthChecker::<MockTransport>::calculate_overall_stats(&results);

		assert_eq!(stats.total_nodes, 0);
		assert_eq!(stats.successful_nodes, 0);
		assert_eq!(stats.overall_success_rate, 0.0);
		assert!(stats.best_node_id.is_none());
		assert!(stats.worst_node_id.is_none());
		assert_eq!(stats.avg_latency_ms, 0.0);
	}
}
