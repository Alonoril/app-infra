use alloy::{
	rpc::json_rpc::{ErrorPayload, Id, Request, RequestPacket, ResponsePacket, SerializedRequest},
	transports::{TransportError, TransportErrorKind, TransportFut},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
	fmt::Debug,
	sync::Arc,
	task::{Context, Poll},
	time::{Duration, Instant},
};
use tokio::sync::OnceCell;
use tower::{Layer, Service};
use tracing::{Instrument, debug, debug_span, info, trace, warn};

use infra_core::rt_pool::Tokio;

use super::{HealthCheckConfig, HealthChecker, ScoredTransport, SerialFailoverConfig, WarmupMethod};

/// 预热结果
#[derive(Debug)]
struct WarmupResult {
	transport_id: usize,
	success: bool,
	latency: Option<Duration>,
	error: Option<String>,
}

/// JSON-RPC error codes that are documented in
/// <https://ethereum-json-rpc.com/errors> and should not be retried.
const ETHEREUM_JSON_RPC_ERROR_CODES: [i64; 11] = [
	-32700, // Parse error
	-32600, // Invalid request
	-32601, // Method not found
	-32602, // Invalid params
	-32603, // Internal error
	-32000, // Server error
	-32002, // Resource unavailable
	-32001, // Resource not found
	-32003, // Transaction rejected
	-32004, // Method not supported
	-32005, // Limit exceeded
];

fn is_fail_fast_rpc_error(err_payload: &ErrorPayload) -> bool {
	if err_payload.code == -32000 {
		let normalized = err_payload.message.as_ref().to_ascii_lowercase();
		if normalized.contains("missing trie node")
			|| (normalized.contains("historical state")
				&& ["unavailable", "not available", "missing"]
					.iter()
					.any(|reason| normalized.contains(reason)))
		{
			return false;
		}
	}

	if ETHEREUM_JSON_RPC_ERROR_CODES.contains(&err_payload.code) {
		return true;
	}

	if err_payload.code == 3 {
		let normalized = err_payload.message.as_ref().to_ascii_lowercase();
		if normalized.contains("execution reverted") {
			return true;
		}
	}

	false
}

/// SerialFailoverLayer 用于替代 alloy 官方的 FallbackLayer
///
/// 与 FallbackLayer 的并发请求不同，SerialFailoverLayer 采用循环式串行请求策略：
/// 1. 按分数排序所有节点
/// 2. 从负载均衡起始点开始循环尝试
/// 3. 单个节点失败后立即切换下一个节点
/// 4. 避免并发请求造成的资源浪费
///
/// # Usage
///
/// ```rust
/// use alloy::providers::{Provider, ProviderBuilder};
/// use alloy::rpc::client::RpcClient;
/// use alloy::transports::http::Http;
/// use alloy_ext::transports::layers::SerialFailoverLayer;
/// use tower::ServiceBuilder;
/// use url::Url;
///
/// async fn demo() -> anyhow::Result<()> {
///    let serial_layer = SerialFailoverLayer::default()
///     .with_stability_weight(0.4)         // 稳定性权重 40%
///     .with_latency_weight(0.3)           // 延迟权重 30%
///     .with_health_weight(0.2)            // 健康权重 20%
///     .with_history_weight(0.1)           // 历史权重 10%
///     .with_warmup_enabled(true);         // 启用懒加载预热
///   let transports = vec![
///         Http::new(Url::parse("https://reth-ethereum.ithaca.xyz/rpc")?),
///         Http::new(Url::parse("https://eth.llamarpc.com")?),
///         Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
///     ];
///    let transport = ServiceBuilder::new().layer(serial_layer).service(transports);
///    let client = RpcClient::builder().transport(transport, false);
///    let provider = ProviderBuilder::new().connect_client(client);
///
///    // 使用标准 alloy Provider API
///    // 预热将在第一次请求时自动执行
///    let latest_block = provider.get_block_number().await?;
///    Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SerialFailoverLayer {
	/// SerialFallback 配置
	config: SerialFailoverConfig,
	/// 健康检查配置（可选）
	health_config: Option<HealthCheckConfig>,
}

impl SerialFailoverLayer {
	/// 创建新的 SerialFailoverLayer
	pub fn new() -> Self {
		let _span = debug_span!("serial_failover_layer_new").entered();
		debug!(target: "serial_failover_layer", "Creating new SerialFailoverLayer");
		Self {
			config: SerialFailoverConfig::default(),
			health_config: None,
		}
	}

	/// 使用指定配置创建 SerialFailoverLayer
	pub fn with_config(config: SerialFailoverConfig) -> Self {
		Self {
			config,
			health_config: None,
		}
	}

	/// 设置稳定性权重
	pub fn with_stability_weight(mut self, weight: f64) -> Self {
		self.config.stability_weight = weight;
		self
	}

	/// 设置延迟权重
	pub fn with_latency_weight(mut self, weight: f64) -> Self {
		self.config.latency_weight = weight;
		self
	}

	/// 设置健康权重
	pub fn with_health_weight(mut self, weight: f64) -> Self {
		self.config.health_weight = weight;
		self
	}

	/// 设置历史权重
	pub fn with_history_weight(mut self, weight: f64) -> Self {
		self.config.history_weight = weight;
		self
	}

	/// 设置样本窗口大小
	pub fn with_sample_count(mut self, count: usize) -> Self {
		self.config.sample_count = count;
		self
	}

	/// 设置不健康阈值
	pub fn with_unhealthy_threshold(mut self, threshold: u32) -> Self {
		self.config.unhealthy_threshold = threshold;
		self
	}

	/// 设置惩罚阈值
	pub fn with_penalty_threshold(mut self, threshold: u32) -> Self {
		self.config.penalty_threshold = threshold;
		self
	}

	/// 设置惩罚系数
	pub fn with_penalty_factor(mut self, factor: f64) -> Self {
		self.config.penalty_factor = factor;
		self
	}

	/// 启用详细日志
	pub fn with_verbose_logging(mut self, enabled: bool) -> Self {
		self.config.enable_verbose_logging = enabled;
		self
	}

	/// 设置是否优先使用最佳节点
	pub fn with_prefer_best_node(mut self, prefer: bool) -> Self {
		self.config.prefer_best_node = prefer;
		self
	}

	/// 设置节点冷却时间（秒）
	pub fn with_node_cooldown_secs(mut self, secs: u64) -> Self {
		self.config.node_cooldown_secs = secs;
		self
	}

	/// 设置是否启用预热
	pub fn with_warmup_enabled(mut self, enabled: bool) -> Self {
		self.config.enable_warmup = enabled;
		self
	}

	/// 设置预热请求次数
	pub fn with_warmup_requests(mut self, requests: u32) -> Self {
		self.config.warmup_requests = requests;
		self
	}

	/// 设置预热超时时间（秒）
	pub fn with_warmup_timeout_secs(mut self, secs: u64) -> Self {
		self.config.warmup_timeout_secs = secs;
		self
	}

	/// 设置预热方法
	pub fn with_warmup_method(mut self, method: WarmupMethod) -> Self {
		self.config.warmup_method = method;
		self
	}

	/// 设置健康检查配置（启用自动健康检查）
	pub fn with_health_check(mut self, health_config: HealthCheckConfig) -> Self {
		self.health_config = Some(health_config);
		self
	}

	/// 启用默认健康检查
	pub fn with_default_health_check(mut self) -> Self {
		self.health_config = Some(HealthCheckConfig::default());
		self
	}

	/// 获取配置的引用
	pub fn config(&self) -> &SerialFailoverConfig {
		&self.config
	}

	/// 获取健康检查配置的引用
	pub fn health_config(&self) -> Option<&HealthCheckConfig> {
		self.health_config.as_ref()
	}

	/// 异步创建带预热的 SerialFallbackService
	pub async fn create_service_with_warmup<S>(&self, transports: Vec<S>) -> SerialFallbackService<S>
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
			+ Send
			+ Sync
			+ Clone
			+ Debug
			+ 'static,
		S::Future: Send + 'static,
	{
		SerialFallbackService::new_with_warmup_and_health_check(
			transports,
			self.config.clone(),
			self.health_config.clone(),
		)
		.await
	}
}

impl Default for SerialFailoverLayer {
	fn default() -> Self {
		Self::new()
	}
}

impl<S> Layer<Vec<S>> for SerialFailoverLayer
where
	S: Service<RequestPacket, Response = ResponsePacket, Future = TransportFut<'static>, Error = TransportError>
		+ Send
		+ Sync
		+ Clone
		+ Debug
		+ 'static,
{
	type Service = SerialFallbackService<S>;

	fn layer(&self, transports: Vec<S>) -> Self::Service {
		SerialFallbackService::new_with_health_check(transports, self.config.clone(), self.health_config.clone())
	}
}

/// SerialFallback Service 实现 Tower Service trait
///
/// 这是实际处理请求的服务，内部使用循环式请求策略
/// 支持懒加载预热：在第一次请求时自动执行预热
#[derive(Debug)]
pub struct SerialFallbackService<S> {
	/// 被分数管理的传输节点列表
	transports: Arc<Vec<Arc<ScoredTransport<S>>>>,
	/// 循环选择的起始索引，用于负载均衡
	round_robin_index: Arc<parking_lot::Mutex<usize>>,
	/// 配置信息
	config: SerialFailoverConfig,
	/// 预热状态：确保预热只执行一次
	warmup_once: Arc<OnceCell<bool>>,
	/// 健康检查器（可选）
	health_checker: Option<Arc<HealthChecker<S>>>,
}

impl<S: Clone + Debug> Clone for SerialFallbackService<S> {
	fn clone(&self) -> Self {
		Self {
			transports: Arc::clone(&self.transports),
			round_robin_index: Arc::clone(&self.round_robin_index),
			config: self.config.clone(),
			warmup_once: Arc::clone(&self.warmup_once),
			health_checker: self.health_checker.clone(),
		}
	}
}

impl<S: Clone + Debug> SerialFallbackService<S> {
	/// 创建新的 SerialFallbackService
	pub fn new(transports: Vec<S>, config: SerialFailoverConfig) -> Self {
		let scored_transports = Self::build_scored_transports(transports, &config);
		Self {
			transports: Arc::new(scored_transports),
			round_robin_index: Arc::new(parking_lot::Mutex::new(0)),
			config,
			warmup_once: Arc::new(OnceCell::new()),
			health_checker: None,
		}
	}

	/// 创建新的 SerialFallbackService（支持健康检查）
	pub fn new_with_health_check(
		transports: Vec<S>,
		config: SerialFailoverConfig,
		health_config: Option<HealthCheckConfig>,
	) -> Self
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
			+ Send
			+ Sync
			+ Clone
			+ Debug
			+ 'static,
		S::Future: Send + 'static,
	{
		let _span = debug_span!(
			"serial_failover_service_new_with_health_check",
			transport_count = transports.len(),
			has_health_config = health_config.is_some()
		)
		.entered();

		let scored_transports = Self::build_scored_transports(transports, &config);

		// 创建健康检查器（如果配置了）
		let health_checker = health_config.map(|hc| {
			let checker = Arc::new(HealthChecker::new(hc));
			// 注册所有节点到健康检查器
			for transport in &scored_transports {
				checker.register_node(transport.id, transport);
			}

			// 启动健康检查器
			let checker_clone = checker.clone();
			Tokio.spawn(async move {
				if let Err(e) = checker_clone.start().await {
					warn!("Failed to start health checker: {}", e);
				}
			});
			checker
		});

		Self {
			transports: Arc::new(scored_transports),
			round_robin_index: Arc::new(parking_lot::Mutex::new(0)),
			config,
			warmup_once: Arc::new(OnceCell::new()),
			health_checker,
		}
	}

	/// 创建新的 SerialFallbackService 并执行预热
	pub async fn new_with_warmup(transports: Vec<S>, config: SerialFailoverConfig) -> Self
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
			+ Send
			+ Sync
			+ Clone
			+ Debug
			+ 'static,
		S::Future: Send + 'static,
	{
		Self::new_with_warmup_and_health_check(transports, config, None).await
	}

	/// 创建新的 SerialFallbackService 并执行预热（支持健康检查）
	pub async fn new_with_warmup_and_health_check(
		transports: Vec<S>,
		config: SerialFailoverConfig,
		health_config: Option<HealthCheckConfig>,
	) -> Self
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
			+ Send
			+ Sync
			+ Clone
			+ Debug
			+ 'static,
		S::Future: Send + 'static,
	{
		let _span = debug_span!(
			"serial_failover_service_new_with_warmup_and_health_check",
			transport_count = transports.len(),
			has_health_config = health_config.is_some(),
			enable_warmup = config.enable_warmup
		)
		.entered();
		let service = Self::new_with_health_check(transports, config, health_config);

		if service.config.enable_warmup {
			service.warmup_transports().await;
			// 标记预热已完成
			let _ = service.warmup_once.set(true);
		}

		service
	}

	/// 获取健康检查器的引用（如果存在）
	pub fn health_checker(&self) -> Option<&Arc<HealthChecker<S>>> {
		self.health_checker.as_ref()
	}

	/// 检查是否启用了健康检查
	pub fn has_health_checker(&self) -> bool {
		self.health_checker.is_some()
	}

	fn build_scored_transports(transports: Vec<S>, config: &SerialFailoverConfig) -> Vec<Arc<ScoredTransport<S>>> {
		transports
			.into_iter()
			.enumerate()
			.map(|(id, transport)| Arc::new(ScoredTransport::with_config(id, transport, config.clone())))
			.collect::<Vec<_>>()
	}

	/// 获取排序后的节点列表（按分数降序）
	fn get_sorted_transports(&self) -> Vec<Arc<ScoredTransport<S>>> {
		let mut transports = (*self.transports).clone();
		transports.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap_or(std::cmp::Ordering::Equal));
		transports
	}

	/// 记录传输排名信息
	fn log_transport_rankings(&self)
	where
		S: Debug,
	{
		let sorted_transports = self.get_sorted_transports();

		debug!(
			target: "serial_failover_layer_rankings",
			"📊 Current node rankings ({} total nodes):",
			sorted_transports.len()
		);

		for (idx, transport) in sorted_transports.iter().enumerate().take(5) {
			let status = if transport.is_cooling_down() {
				"🧊 Cooling"
			} else if transport.is_healthy() {
				"✅ Healthy"
			} else {
				"❌ Unhealthy"
			};

			debug!(
				target: "serial_failover_layer_rankings",
				"  #{}: Node[{}] | {} | Status: {}",
				idx + 1,
				transport.id,
				transport.metrics_summary(),
				status
			);
		}

		// 如果有超过5个节点，显示总结信息
		if sorted_transports.len() > 5 {
			let healthy_count = sorted_transports.iter().filter(|t| t.is_healthy()).count();
			let cooling_count = sorted_transports.iter().filter(|t| t.is_cooling_down()).count();
			debug!(
				target: "serial_failover_layer_rankings",
				"  ... {} more nodes | Healthy: {} | Cooling: {} | Unhealthy: {}",
				sorted_transports.len() - 5,
				healthy_count,
				cooling_count,
				sorted_transports.len() - healthy_count - cooling_count
			);
		}
	}

	/// 选择起始节点索引
	///
	/// 根据配置决定使用"延迟最低优先"还是"负载均衡"策略
	fn select_start_index(&self, available_count: usize) -> usize {
		if available_count == 0 {
			return 0;
		}

		if self.config.prefer_best_node {
			// 延迟最低优先策略：总是从分数最高的节点开始
			0
		} else {
			// 传统的负载均衡策略：轮询选择起始点
			let mut index = self.round_robin_index.lock();
			let current = *index;
			*index = (current + 1) % available_count;
			current
		}
	}

	/// 懒加载预热：如果启用预热且尚未完成，则执行预热
	async fn ensure_warmup(&self)
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError> + Send + Sync + 'static,
		S::Future: Send + 'static,
	{
		if !self.config.enable_warmup {
			return;
		}

		// 使用 OnceCell 确保预热只执行一次
		let warmup_once = Arc::clone(&self.warmup_once);
		let _ = warmup_once
			.get_or_init(|| async {
				info!(
					target: "serial_failover_warmup",
					"🔥 Performing lazy warmup for {} transports using method: {:?}",
					self.transports.len(),
					self.config.warmup_method
				);

				self.warmup_transports().await;
				true
			})
			.await;
	}

	/// 执行节点预热
	async fn warmup_transports(&self)
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError> + Send + Sync + 'static,
		S::Future: Send + 'static,
	{
		async move {
			let transport_count = self.transports.len();
			if transport_count == 0 {
				return;
			}

			info!(
				target: "serial_failover_warmup",
				"🔥 Starting warmup for {} transports using method: {:?}",
				transport_count,
				self.config.warmup_method
			);

			// 创建预热请求
			let warmup_request = self.create_warmup_request();
			let timeout_duration = self.config.warmup_timeout_duration();

			// 并发执行预热
			let mut warmup_tasks = Vec::new();
			for transport in self.transports.iter() {
				let transport_clone = Arc::clone(transport);
				let request_clone = warmup_request.clone();
				let warmup_requests = self.config.warmup_requests;

				let task = Tokio.spawn(async move {
					Self::warmup_single_transport(transport_clone, request_clone, warmup_requests, timeout_duration)
						.await
				});

				warmup_tasks.push(task);
			}

			// 等待所有预热任务完成
			let mut successful_count = 0;
			let mut total_latency = Duration::ZERO;
			let mut results = Vec::new();

			for (idx, task) in warmup_tasks.into_iter().enumerate() {
				match task.await {
					Ok(result) => {
						if result.success {
							successful_count += 1;
							if let Some(latency) = result.latency {
								total_latency += latency;
							}
						}
						results.push(result);
					}
					Err(e) => {
						let result = WarmupResult {
							transport_id: idx,
							success: false,
							latency: None,
							error: Some(format!("Task join error: {}", e)),
						};
						results.push(result);
					}
				}
			}

			// 记录预热结果
			self.log_warmup_results(&results, successful_count, total_latency);
		}
		.instrument(debug_span!("serial_failover_warmup_transports",
			transport_count = self.transports.len(),
			warmup_method = ?self.config.warmup_method,
			warmup_requests = self.config.warmup_requests
		))
		.await
	}

	/// 创建预热请求
	fn create_warmup_request(&self) -> RequestPacket {
		let (method, params) = match self.config.warmup_method {
			WarmupMethod::BlockNumber => ("eth_blockNumber", Value::Array(vec![])),
			WarmupMethod::ChainId => ("eth_chainId", Value::Array(vec![])),
			WarmupMethod::NetVersion => ("net_version", Value::Array(vec![])),
		};

		let request = Request::new(method, Id::Number(1), params);

		// 使用 Request::serialize() 方法创建 SerializedRequest
		match request.serialize() {
			Ok(serialized) => RequestPacket::Single(serialized),
			Err(_) => {
				// 如果序列化失败，创建一个备用的简单请求
				let fallback_request = Request::new("eth_blockNumber", Id::Number(1), Value::Array(vec![]));
				fallback_request
					.serialize()
					.map(RequestPacket::Single)
					.unwrap_or_else(|_| {
						// 记录错误但不 panic，返回一个默认的简单请求包
						warn!(target: "serial_failover_warmup", "Failed to serialize warmup request, using fallback");
						// 使用最简单的 eth_blockNumber 方法
						let simple_request = Request::new("eth_blockNumber", Id::Number(1), ());
						match simple_request.serialize() {
							Ok(serialized) => RequestPacket::Single(serialized),
							Err(_) => {
								// 如果连简单请求都失败了，使用手动构建
								let json_rpc = serde_json::json!({
									"jsonrpc": "2.0",
									"id": 1,
									"method": "eth_blockNumber",
									"params": []
								});
								let serialized: Request<Value> =
									Request::deserialize(json_rpc).expect("Failed to create fallback request");
								RequestPacket::Single(SerializedRequest::try_from(serialized).unwrap())
							}
						}
					})
			}
		}
	}

	/// 预热单个传输节点
	async fn warmup_single_transport(
		transport: Arc<ScoredTransport<S>>,
		request: RequestPacket,
		warmup_requests: u32,
		timeout: Duration,
	) -> WarmupResult
	where
		S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError> + Send + Sync + 'static,
		S::Future: Send + 'static,
	{
		let transport_id = transport.id;
		async move {
			let mut total_latency = Duration::ZERO;
			let mut successful_requests = 0u32;
			let mut last_error = None;

			for _ in 0..warmup_requests {
				let mut inner_transport = transport.transport.clone();
				let request_clone = request.clone();

				let start_time = Instant::now();

				// 使用 tokio::time::timeout 添加超时
				match tokio::time::timeout(timeout, inner_transport.call(request_clone)).await {
					Ok(Ok(_)) => {
						let latency = start_time.elapsed();
						total_latency += latency;
						successful_requests += 1;

						// 记录成功的预热请求
						transport.track_success(latency);

						trace!(
							target: "serial_failover_warmup",
							"Transport[{}] warmup success: latency={:?}",
							transport.id,
							latency
						);
					}
					Ok(Err(e)) => {
						// 记录失败的预热请求
						transport.track_failure();
						last_error = Some(e.to_string());

						trace!(
							target: "serial_failover_warmup",
							"Transport[{}] warmup failed: {}",
							transport.id,
							e
						);
					}
					Err(_) => {
						// 超时
						transport.track_failure();
						last_error = Some(format!("Timeout after {:?}", timeout));

						trace!(
							target: "serial_failover_warmup",
							"Transport[{}] warmup timeout after {:?}",
							transport.id,
							timeout
						);
					}
				}
			}

			let success = successful_requests > 0;
			let avg_latency = if successful_requests > 0 {
				Some(total_latency / successful_requests)
			} else {
				None
			};

			WarmupResult {
				transport_id,
				success,
				latency: avg_latency,
				error: if success { None } else { last_error },
			}
		}
		.instrument(debug_span!("serial_failover_warmup_single_transport",
			transport_id = %transport_id,
			warmup_requests = %warmup_requests,
			timeout_ms = %timeout.as_millis()
		))
		.await
	}

	/// 记录预热结果
	fn log_warmup_results(&self, results: &[WarmupResult], successful_count: usize, total_latency: Duration) {
		let total_count = results.len();
		let avg_latency = if successful_count > 0 {
			total_latency / successful_count as u32
		} else {
			Duration::ZERO
		};

		info!(
			target: "serial_failover_warmup",
			"🎯 Warmup completed: {}/{} transports successful, avg latency: {:?}",
			successful_count,
			total_count,
			avg_latency
		);

		// 显示每个节点的预热结果
		for result in results {
			match (result.success, &result.latency) {
				(true, Some(latency)) => {
					info!(
						target: "serial_failover_warmup",
						"  ✅ Transport[{}]: latency={:?}",
						result.transport_id,
						latency
					);
				}
				(false, _) => {
					let error_msg = result.error.as_deref().unwrap_or("Unknown error");
					warn!(
						target: "serial_failover_warmup",
						"  ❌ Transport[{}]: {}",
						result.transport_id,
						error_msg
					);
				}
				_ => {}
			}
		}

		// 显示预热后的排名
		if self.config.enable_verbose_logging {
			self.log_transport_rankings();
		}
	}
}

impl<S> SerialFallbackService<S>
where
	S: Service<RequestPacket, Response = ResponsePacket, Future = TransportFut<'static>, Error = TransportError>
		+ Send
		+ Sync
		+ Clone
		+ Debug
		+ 'static,
{
	/// 执行循环式请求
	///
	/// 核心逻辑：
	/// 1. 首先执行懒加载预热（如果需要）
	/// 2. 按分数排序所有节点
	/// 3. 从负载均衡起始点开始循环尝试
	/// 4. 单个节点失败后立即切换
	/// 5. 记录每个节点的响应时间和成功率
	async fn make_request(&self, req: RequestPacket) -> Result<ResponsePacket, TransportError> {
		async move {
			// 执行懒加载预热
			self.ensure_warmup().await;
			let sorted_transports = self.get_sorted_transports();
			if sorted_transports.is_empty() {
				return Err(TransportErrorKind::custom_str("No transports available"));
			}

			// 根据配置选择起始策略
			let start_index = self.select_start_index(sorted_transports.len());
			if self.config.enable_verbose_logging {
				debug!(
					target: "serial_failover_layer",
					"Request strategy: {} | Available nodes: {} | Starting from index: {}",
					if self.config.prefer_best_node { "Best-First" } else { "Round-Robin" },
					sorted_transports.len(),
					start_index
				);
			}

			let max_cycle_attempts = 3;
			let mut last_error = None;
			let mut cycle_start_index = start_index;

			for cycle in 1..=max_cycle_attempts {
				let mut tried_count = 0;

				// 循环尝试所有节点
				for i in 0..sorted_transports.len() {
					let transport_index = (cycle_start_index + i) % sorted_transports.len();
					let transport = &sorted_transports[transport_index];

					// 跳过不健康的节点
					if !transport.is_healthy() {
						if self.config.enable_verbose_logging {
							if transport.is_cooling_down() {
								warn!(
									target: "serial_failover_layer",
									"Skipping transport[{}] - cooling down | Score: {:.4}",
									transport.id,
									transport.score()
								);
							} else {
								debug!(
									target: "serial_failover_layer",
									"Skipping transport[{}] - unhealthy | Score: {:.4}",
									transport.id,
									transport.score()
								);
							}
						}
						continue;
					}

					tried_count += 1;
					let mut inner_transport = transport.transport.clone();
					let req_clone = req.clone();

					debug!(
						target: "serial_failover_layer",
						"Trying transport[{}] (attempt {}/{}) | Score: {:.4} | Strategy: {}",
						transport.id,
						tried_count,
						sorted_transports.len(),
						transport.score(),
						if self.config.prefer_best_node { "Best-First" } else { "Round-Robin" }
					);

					let start_time = Instant::now();
					match inner_transport.call(req_clone).await {
						Ok(response) => {
							let duration = start_time.elapsed();

							// 记录成功
							transport.track_success(duration);

							// 内部错误不能忽视
							if let Some(err_payload) = response.as_error() {
								let rpc_error = TransportError::ErrorResp(err_payload.clone());
								if is_fail_fast_rpc_error(err_payload) {
									debug!(
										target: "serial_failover_layer",
										"Fail-fast RPC error: code={} message={} | node={}",
										err_payload.code,
										err_payload.message,
										transport.id,
									);
									return Err(rpc_error);
								}

								transport.track_failure();
								warn!(
									target: "serial_failover_layer",
									"Internal error: Transport[{}] | {err_payload}",
									transport.id,
								);

								last_error = Some(rpc_error);

								// 立即尝试下一个节点，不等待
								continue;
							}

							// 记录请求成功的关键信息（只在需要时记录）
							if tried_count > 1 || duration.as_millis() > 1000 {
								info!(
									target: "serial_failover_layer",
									"✅ Request successful: Node[{}] | latency={:?} | attempts={} | score={:.4}",
									transport.id, duration, tried_count, transport.score()
								);
							}

							trace!(
								target: "serial_failover_layer",
								"Transport[{}] succeeded: latency={:?}",
								transport.id, duration
							);

							// 定期打印排名信息
							if transport.should_log_rankings() {
								self.log_transport_rankings();
							}

							return Ok(response);
						}
						Err(error) => {
							// 记录失败
							transport.track_failure();

							warn!(
								target: "serial_failover_layer",
								"Transport[{}] (attempt {}/{}) failed: {}",
								transport.id,
								tried_count,
								sorted_transports.len(),
								error,
							);

							if let Some(err_payload) = error.as_error_resp()
								&& is_fail_fast_rpc_error(err_payload)
							{
								debug!(
									target: "serial_failover_layer",
									"Fail-fast RPC error (transport): code={} message={} | node={}",
									err_payload.code,
									err_payload.message,
									transport.id,
								);
								return Err(error);
							}

							last_error = Some(error);

							// 立即尝试下一个节点，不等待
							continue;
						}
					}
				}

				// 所有节点都失败了
				if tried_count == 0 {
					warn!(
						target: "serial_failover_layer",
						"❌ Request failed: No healthy nodes available | total_nodes={} | strategy={}",
						sorted_transports.len(),
						if self.config.prefer_best_node { "Best-First" } else { "Round-Robin" }
					);
					return Err(TransportErrorKind::custom_str("No healthy transports available"));
				}

				if cycle < max_cycle_attempts {
					cycle_start_index = (cycle_start_index + 1) % sorted_transports.len();
					let next_start_node = &sorted_transports[cycle_start_index];
					warn!(
						target: "serial_failover_layer",
						"⚠️ All nodes failed in cycle {}/{} | tried_nodes={} | retrying with Node[{}] next | strategy={}",
						cycle,
						max_cycle_attempts,
						tried_count,
						next_start_node.id,
						if self.config.prefer_best_node { "Best-First" } else { "Round-Robin" }
					);
					last_error = None;
					continue;
				}

				// 记录所有节点都失败的关键信息（最终失败）
				warn!(
					target: "serial_failover_layer",
					"❌ Request failed: All nodes failed in cycle={} | tried_nodes={} | total_nodes={} | strategy={}",
					max_cycle_attempts,
					tried_count,
					sorted_transports.len(),
					if self.config.prefer_best_node { "Best-First" } else { "Round-Robin" }
				);

				// 记录最终失败的排名
				self.log_transport_rankings();

				return Err(last_error.unwrap_or_else(|| TransportErrorKind::custom_str("All transports failed")));
			}

			unreachable!("serial failover cycle loop exited without returning");
		}
		.instrument(debug_span!(
			"serial_failover_make_cyclic_request",
			transport_count = self.transports.len(),
			prefer_best_node = self.config.prefer_best_node,
			enable_warmup = self.config.enable_warmup
		))
		.await
	}
}

impl<S> SerialFallbackService<S>
where
	S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError> + Send + Sync + Clone + 'static,
	S::Future: Send + 'static,
{
	/// 手动停止健康检查器（如果存在）
	pub async fn stop_health_checker(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
		if let Some(ref health_checker) = self.health_checker {
			health_checker
				.stop()
				.await
				.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
		}
		Ok(())
	}
}

impl<S> Service<RequestPacket> for SerialFallbackService<S>
where
	S: Service<RequestPacket, Response = ResponsePacket, Future = TransportFut<'static>, Error = TransportError>
		+ Send
		+ Sync
		+ Clone
		+ Debug
		+ 'static,
{
	type Response = ResponsePacket;
	type Error = TransportError;
	type Future = TransportFut<'static>;

	fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: RequestPacket) -> Self::Future {
		let _span = debug_span!(
			"serial_failover_service_call",
			transport_count = self.transports.len(),
			has_health_checker = self.health_checker.is_some()
		);

		let this = self.clone();
		Box::pin(async move { this.make_request(req).await }.instrument(_span))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::borrow::Cow;

	#[test]
	fn historical_state_rpc_errors_are_failover_candidates() {
		let error = ErrorPayload {
			code: -32000,
			message: Cow::Borrowed("missing trie node 0xabc"),
			data: None,
		};

		assert!(!is_fail_fast_rpc_error(&error));
	}

	#[test]
	fn unrelated_rpc_server_errors_remain_fail_fast() {
		let error = ErrorPayload {
			code: -32000,
			message: Cow::Borrowed("insufficient funds for transfer"),
			data: None,
		};

		assert!(is_fail_fast_rpc_error(&error));
	}

	#[tokio::test]
	async fn test_serial_failover_layer_creation() {
		let layer = SerialFailoverLayer::new();
		assert!(layer.config().stability_weight > 0.0);

		let layer_with_config = SerialFailoverLayer::default()
			.with_stability_weight(0.5)
			.with_latency_weight(0.3)
			.with_health_weight(0.2);

		assert_eq!(layer_with_config.config().stability_weight, 0.5);
		assert_eq!(layer_with_config.config().latency_weight, 0.3);
		assert_eq!(layer_with_config.config().health_weight, 0.2);
	}

	#[tokio::test]
	async fn test_smart_provider_service_creation() {
		#[derive(Debug, Clone)]
		struct MockTransport;

		let transports: Vec<MockTransport> = vec![MockTransport, MockTransport, MockTransport];
		let config = SerialFailoverConfig::default();

		let service = SerialFallbackService::new(transports, config);
		assert_eq!(service.transports.len(), 3);
	}

	#[test]
	fn test_layer_trait_implementation() {
		use alloy::{
			rpc::json_rpc::{RequestPacket, ResponsePacket},
			transports::{TransportError, TransportErrorKind, TransportFut},
		};
		use std::task::{Context, Poll};
		use tower::Service;

		// 创建一个简单的模拟 Transport
		#[derive(Debug, Clone)]
		struct MockTransport;

		impl Service<RequestPacket> for MockTransport {
			type Response = ResponsePacket;
			type Error = TransportError;
			type Future = TransportFut<'static>;

			fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
				Poll::Ready(Ok(()))
			}

			fn call(&mut self, _req: RequestPacket) -> Self::Future {
				Box::pin(async { Err(TransportErrorKind::custom_str("Mock error")) })
			}
		}

		let layer = SerialFailoverLayer::default();
		let transports: Vec<MockTransport> = vec![MockTransport, MockTransport];

		// 验证 Layer trait 实现正确
		let _service = layer.layer(transports);
	}
}
