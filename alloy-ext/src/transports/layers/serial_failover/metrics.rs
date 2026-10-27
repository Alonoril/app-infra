use crate::transports::layers::{ScoringAlgorithm, SerialFailoverConfig};
use alloy::rpc::json_rpc::{RequestPacket, ResponsePacket};
use alloy::transports::{TransportError, TransportFut};
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tower_service::Service;
use tracing::debug;

/// 带分数的传输节点
///
/// 封装了单个传输节点及其性能指标，
/// 支持基于延迟和稳定性的分数计算
#[derive(Debug, Clone)]
pub struct ScoredTransport<S> {
	/// 传输节点本身
	pub transport: S,
	/// 节点唯一标识
	pub id: usize,
	/// 性能指标
	pub(crate) metrics: Arc<RwLock<TransportMetrics>>,
}

impl<S> ScoredTransport<S> {
	/// 创建新的分数化传输节点
	pub fn new(id: usize, transport: S) -> Self {
		Self::with_config(id, transport, SerialFailoverConfig::default())
	}

	/// 使用指定配置创建分数化传输节点
	pub fn with_config(id: usize, transport: S, config: SerialFailoverConfig) -> Self {
		Self {
			transport,
			id,
			metrics: Arc::new(RwLock::new(TransportMetrics::with_config(config))),
		}
	}

	/// 获取当前分数
	pub fn score(&self) -> f64 {
		let metrics = self.metrics.read();
		metrics.calculate_score()
	}

	/// 检查节点是否健康（考虑冷却状态）
	pub fn is_healthy(&self) -> bool {
		let metrics = self.metrics.read();
		metrics.is_healthy()
	}

	/// 检查节点是否在冷却期
	pub fn is_cooling_down(&self) -> bool {
		let metrics = self.metrics.read();
		metrics.is_cooling_down()
	}

	/// 检查节点是否可用（不考虑冷却状态）
	pub fn is_available(&self) -> bool {
		let metrics = self.metrics.read();
		metrics.is_available()
	}

	/// 获取指标摘要
	pub fn metrics_summary(&self) -> String {
		let metrics = self.metrics.read();
		metrics.get_summary()
	}

	/// 记录成功请求
	pub fn track_success(&self, duration: Duration) {
		let mut metrics = self.metrics.write();
		metrics.track_success(duration);
	}

	/// 记录失败请求
	pub fn track_failure(&self) {
		let mut metrics = self.metrics.write();
		metrics.track_failure();
	}

	/// 记录健康检查成功
	pub fn track_health_success(&self, duration: Duration) {
		let mut metrics = self.metrics.write();
		metrics.track_health_success(duration);
	}

	/// 记录健康检查失败
	pub fn track_health_failure(&self) {
		let mut metrics = self.metrics.write();
		metrics.track_health_failure();
	}

	/// 是否应该记录排名（减少日志噪音）
	pub fn should_log_rankings(&self) -> bool {
		let metrics = self.metrics.read();
		metrics.should_log_rankings()
	}
}

impl<S> Service<RequestPacket> for ScoredTransport<S>
where
	S: Service<RequestPacket, Future = TransportFut<'static>, Error = TransportError>,
{
	type Response = ResponsePacket;
	type Error = TransportError;
	type Future = TransportFut<'static>;

	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		self.transport.poll_ready(cx)
	}

	fn call(&mut self, req: RequestPacket) -> Self::Future {
		self.transport.call(req)
	}
}

/// 传输性能指标
///
/// 跟踪节点的延迟、成功率等关键指标，
/// 用于计算节点分数和健康状态
/// 支持主动/被动数据分离以提供更准确的评分
#[derive(Debug)]
pub struct TransportMetrics {
	/// 被动监测数据（实际RPC请求）
	passive_data: MetricsData,
	/// 主动监测数据（健康检查）
	active_data: MetricsData,
	/// 最后更新时间
	last_update: Instant,
	/// 总请求数（被动）
	total_requests: u64,
	/// 成功请求数（被动）
	successful_requests: u64,
	/// 连续失败计数（被动）
	consecutive_failures: u32,
	/// 上次记录排名的时间
	last_ranking_log: Option<Instant>,
	/// 节点冷却结束时间（连续失败后的暂时排除）
	cooldown_until: Option<Instant>,
	/// 配置信息
	config: SerialFailoverConfig,
}

/// 指标数据容器
///
/// 分别存储主动和被动监测的延迟和成功率数据
#[derive(Debug)]
pub struct MetricsData {
	/// 延迟历史记录（滑动窗口）
	latencies: VecDeque<Duration>,
	/// 成功失败历史记录（滑动窗口）
	successes: VecDeque<bool>,
	/// 总请求数
	total_count: u64,
	/// 成功请求数
	success_count: u64,
}

impl MetricsData {
	/// 创建新的指标数据容器
	pub fn new() -> Self {
		Self {
			latencies: VecDeque::new(),
			successes: VecDeque::new(),
			total_count: 0,
			success_count: 0,
		}
	}

	/// 记录成功请求
	pub fn track_success(&mut self, duration: Duration, window_size: usize) {
		self.total_count += 1;
		self.success_count += 1;

		self.latencies.push_back(duration);
		self.successes.push_back(true);

		// 限制窗口大小
		self.trim_windows(window_size);
	}

	/// 记录失败请求
	pub fn track_failure(&mut self, window_size: usize) {
		self.total_count += 1;
		self.successes.push_back(false);

		// 限制窗口大小
		self.trim_windows(window_size);
	}

	/// 修剪滑动窗口
	fn trim_windows(&mut self, window_size: usize) {
		while self.latencies.len() > window_size {
			self.latencies.pop_front();
		}
		while self.successes.len() > window_size {
			self.successes.pop_front();
		}
	}

	/// 计算成功率
	pub fn success_rate(&self) -> f64 {
		if self.successes.is_empty() {
			return 0.0;
		}
		let success_count = self.successes.iter().filter(|&&s| s).count();
		success_count as f64 / self.successes.len() as f64
	}

	/// 计算平均延迟
	pub fn avg_latency(&self) -> f64 {
		if self.latencies.is_empty() {
			return 0.0;
		}
		self.latencies.iter().map(|d| d.as_secs_f64()).sum::<f64>() / self.latencies.len() as f64
	}

	/// 获取样本数量
	pub fn sample_count(&self) -> usize {
		self.successes.len()
	}

	/// 获取总请求数
	pub fn total_requests(&self) -> u64 {
		self.total_count
	}

	/// 获取成功请求数
	pub fn successful_requests(&self) -> u64 {
		self.success_count
	}
}
impl Default for MetricsData {
	fn default() -> Self {
		Self::new()
	}
}

impl TransportMetrics {
	/// 使用指定配置创建
	pub fn with_config(config: SerialFailoverConfig) -> Self {
		Self {
			passive_data: MetricsData::new(),
			active_data: MetricsData::new(),
			last_update: Instant::now(),
			total_requests: 0,
			successful_requests: 0,
			consecutive_failures: 0,
			last_ranking_log: None,
			cooldown_until: None,
			config,
		}
	}

	/// 记录被动成功请求（实际RPC请求）
	pub fn track_success(&mut self, duration: Duration) {
		let old_score = self.calculate_score();

		self.total_requests += 1;
		self.successful_requests += 1;
		self.consecutive_failures = 0;
		self.last_update = Instant::now();

		// 成功时清除冷却状态
		self.cooldown_until = None;

		// 记录到被动数据
		self.passive_data.track_success(duration, self.config.sample_count);

		// 每10次成功请求记录一次指标更新
		if self.successful_requests.is_multiple_of(10) {
			let new_score = self.calculate_score();
			let score_change = ((new_score - old_score) / old_score.max(0.001)) * 100.0;

			debug!(
				"📈 Node performance metrics updated: success_rate={:.1}% | latency={:.0}ms | score={:.4} ({:+.2}%) | total_requests={}",
				self.passive_data.success_rate() * 100.0,
				self.passive_data.avg_latency() * 1000.0,
				new_score,
				score_change,
				self.total_requests
			);
		}
	}

	/// 记录被动失败请求（实际RPC请求）
	pub fn track_failure(&mut self) {
		self.total_requests += 1;
		self.consecutive_failures += 1;
		self.last_update = Instant::now();

		// 如果连续失败次数超过惩罚阈值，启动冷却
		if self.consecutive_failures >= self.config.penalty_threshold {
			let cooldown_duration = self.config.node_cooldown_duration();
			self.cooldown_until = Some(Instant::now() + cooldown_duration);

			// 节点进入冷却期时记录重要信息
			debug!(
				"⚠️ Node entering cooldown: {} consecutive failures | cooldown_duration={:?} | current_score={:.4} | success_rate={:.1}%",
				self.consecutive_failures,
				cooldown_duration,
				self.calculate_score(),
				self.passive_data.success_rate() * 100.0
			);
		}

		// 记录到被动数据
		self.passive_data.track_failure(self.config.sample_count);
	}

	/// 记录主动成功请求（健康检查）
	pub fn track_health_success(&mut self, duration: Duration) {
		self.last_update = Instant::now();

		// 记录到主动数据
		self.active_data.track_success(duration, self.config.sample_count);
	}

	/// 记录主动失败请求（健康检查）
	pub fn track_health_failure(&mut self) {
		self.last_update = Instant::now();

		// 记录到主动数据
		self.active_data.track_failure(self.config.sample_count);
	}

	/// 获取被动监测数据（实际RPC请求）
	pub fn passive_data(&self) -> &MetricsData {
		&self.passive_data
	}

	/// 获取主动监测数据（健康检查）
	pub fn active_data(&self) -> &MetricsData {
		&self.active_data
	}

	/// 计算综合分数
	///
	/// 使用多维度算法：被动数据（实际RPC请求）、主动数据（健康检查）、历史表现
	pub fn calculate_score(&self) -> f64 {
		match &self.config.scoring.algorithm {
			ScoringAlgorithm::SimpleWeighted => self.calculate_simple_weighted_score(),
			ScoringAlgorithm::ExponentialMovingAverage { alpha } => self.calculate_ema_score(*alpha),
			ScoringAlgorithm::Adaptive => self.calculate_adaptive_score(),
		}
	}

	/// 简单加权评分算法
	fn calculate_simple_weighted_score(&self) -> f64 {
		// 被动数据（实际RPC请求）评分
		let passive_score = if self.passive_data.sample_count() > 0 {
			self.calculate_data_score(&self.passive_data)
		} else {
			0.0
		};

		// 主动数据（健康检查）评分
		let active_score = if self.active_data.sample_count() > 0 {
			self.calculate_data_score(&self.active_data)
		} else {
			0.0
		};

		// 历史表现评分（基于总体统计）
		let history_score = if self.total_requests > 0 {
			self.successful_requests as f64 / self.total_requests as f64
		} else {
			0.0
		};

		// 如果没有任何数据，返回0
		if self.passive_data.sample_count() == 0 && self.active_data.sample_count() == 0 {
			return 0.0;
		}

		// 动态权重调整：优先使用被动数据
		let (passive_weight, active_weight, history_weight) = if self.passive_data.sample_count() > 0 {
			// 有被动数据时的权重分配
			(
				self.config.stability_weight + self.config.latency_weight,
				self.config.health_weight,
				self.config.history_weight,
			)
		} else {
			// 只有主动数据时的权重分配
			(
				0.0,
				self.config.stability_weight + self.config.latency_weight + self.config.health_weight,
				self.config.history_weight,
			)
		};

		// 连续失败惩罚
		let failure_penalty = if self.consecutive_failures > self.config.penalty_threshold {
			self.config.penalty_factor
		} else {
			1.0
		};

		// 延迟惩罚
		let latency_penalty = self.calculate_latency_penalty();

		// 加权计算最终分数
		let base_score =
			(passive_score * passive_weight) + (active_score * active_weight) + (history_score * history_weight);

		base_score * failure_penalty * latency_penalty
	}

	/// 指数移动平均评分算法
	fn calculate_ema_score(&self, _alpha: f64) -> f64 {
		// 简化实现：基于简单加权分数进行指数平滑
		let current_score = self.calculate_simple_weighted_score();

		// 如果是第一次计算，直接返回当前分数
		if self.total_requests <= 1 {
			return current_score;
		}

		// 使用指数移动平均平滑分数
		// 这里简化实现，实际应该存储历史EMA值
		current_score
	}

	/// 自适应评分算法
	fn calculate_adaptive_score(&self) -> f64 {
		let base_score = self.calculate_simple_weighted_score();

		// 根据节点表现动态调整
		let performance_bonus = if self.consecutive_failures == 0 && self.passive_data.success_rate() > 0.95 {
			1.1 // 10%奖励
		} else if self.consecutive_failures > self.config.penalty_threshold * 2 {
			0.8 // 20%惩罚
		} else {
			1.0
		};

		base_score * performance_bonus
	}

	/// 计算单个数据源的分数
	fn calculate_data_score(&self, data: &MetricsData) -> f64 {
		let stability_score = data.success_rate();
		let latency_score = if data.sample_count() > 0 {
			let avg_latency = data.avg_latency();
			1.0 / (1.0 + avg_latency)
		} else {
			0.0
		};

		// 简单平均（在多维度评分中会被外层权重调整）
		(stability_score + latency_score) / 2.0
	}

	/// 计算延迟惩罚
	fn calculate_latency_penalty(&self) -> f64 {
		let primary_data = if self.passive_data.sample_count() > 0 {
			&self.passive_data
		} else {
			&self.active_data
		};

		if primary_data.sample_count() == 0 {
			return 1.0;
		}

		let avg_latency_ms = primary_data.avg_latency() * 1000.0;
		let threshold = self.config.scoring.latency_sensitivity.threshold_ms as f64;

		if avg_latency_ms > threshold {
			self.config.scoring.latency_sensitivity.penalty_factor
		} else {
			1.0
		}
	}

	/// 检查节点是否在冷却期
	pub fn is_cooling_down(&self) -> bool {
		if let Some(cooldown_until) = self.cooldown_until {
			Instant::now() < cooldown_until
		} else {
			false
		}
	}

	/// 检查节点是否健康（考虑冷却状态）
	pub fn is_healthy(&self) -> bool {
		// 如果在冷却期，认为不健康
		if self.is_cooling_down() {
			return false;
		}

		// 检查传统的健康状态
		self.consecutive_failures < self.config.unhealthy_threshold
	}

	/// 检查节点是否可用（不考虑冷却状态的健康状态）
	pub fn is_available(&self) -> bool {
		self.consecutive_failures < self.config.unhealthy_threshold
	}

	/// 获取指标摘要
	pub fn get_summary(&self) -> String {
		// 优先使用被动数据
		let primary_data = if self.passive_data.sample_count() > 0 {
			&self.passive_data
		} else {
			&self.active_data
		};

		let success_rate = primary_data.success_rate();
		let avg_latency = primary_data.avg_latency();

		format!(
			"success_rate: {:.2}%, avg_latency: {:.2}ms, passive_samples: {}, active_samples: {}, score: {:.4}, consecutive_failures: {}",
			success_rate * 100.0,
			avg_latency * 1000.0,
			self.passive_data.sample_count(),
			self.active_data.sample_count(),
			self.calculate_score(),
			self.consecutive_failures
		)
	}

	/// 是否应该记录排名
	pub fn should_log_rankings(&self) -> bool {
		let now = Instant::now();
		match self.last_ranking_log {
			Some(last_log) => now.duration_since(last_log) > self.config.ranking_log_interval(),
			None => true,
		}
	}
}

impl Default for TransportMetrics {
	fn default() -> Self {
		Self::with_config(SerialFailoverConfig::default())
	}
}
