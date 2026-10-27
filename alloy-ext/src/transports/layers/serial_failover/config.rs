use crate::error::AyeErr;
use infra_core::{err, map_err, result::AppResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 多维度评分配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
	/// 评分算法类型
	#[serde(default)]
	pub algorithm: ScoringAlgorithm,

	/// 是否启用自适应权重调整
	#[serde(default = "default_adaptive_weights")]
	pub adaptive_weights: bool,

	/// 最小稳定期（秒）- 新节点需要稳定运行的时间
	#[serde(default = "default_stabilization_period")]
	pub stabilization_period_secs: u64,

	/// 延迟敏感度配置
	#[serde(default)]
	pub latency_sensitivity: LatencySensitivity,

	/// 分数衰减配置
	#[serde(default)]
	pub score_decay: ScoreDecayConfig,
}

/// 评分算法类型
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum ScoringAlgorithm {
	/// 简单加权平均
	#[default]
	SimpleWeighted,
	/// 指数移动平均
	ExponentialMovingAverage { alpha: f64 },
	/// 自适应算法（根据节点表现动态调整）
	Adaptive,
}

/// 延迟敏感度配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencySensitivity {
	/// 延迟阈值（毫秒）- 超过此值的延迟将被重度惩罚
	#[serde(default = "default_latency_threshold")]
	pub threshold_ms: u64,

	/// 延迟惩罚因子
	#[serde(default = "default_latency_penalty_factor")]
	pub penalty_factor: f64,

	/// 是否启用延迟分级
	#[serde(default = "default_enable_latency_tiers")]
	pub enable_tiers: bool,
}

/// 分数衰减配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDecayConfig {
	/// 是否启用分数衰减
	#[serde(default = "default_enable_decay")]
	pub enabled: bool,

	/// 衰减因子 (0.0-1.0)
	#[serde(default = "default_decay_factor")]
	pub factor: f64,

	/// 衰减间隔（秒）
	#[serde(default = "default_decay_interval")]
	pub interval_secs: u64,
}

impl Default for ScoringConfig {
	fn default() -> Self {
		Self {
			algorithm: ScoringAlgorithm::default(),
			adaptive_weights: default_adaptive_weights(),
			stabilization_period_secs: default_stabilization_period(),
			latency_sensitivity: LatencySensitivity::default(),
			score_decay: ScoreDecayConfig::default(),
		}
	}
}

impl Default for LatencySensitivity {
	fn default() -> Self {
		Self {
			threshold_ms: default_latency_threshold(),
			penalty_factor: default_latency_penalty_factor(),
			enable_tiers: default_enable_latency_tiers(),
		}
	}
}

impl Default for ScoreDecayConfig {
	fn default() -> Self {
		Self {
			enabled: default_enable_decay(),
			factor: default_decay_factor(),
			interval_secs: default_decay_interval(),
		}
	}
}

/// 预热方法枚举
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum WarmupMethod {
	/// 使用 eth_blockNumber 方法进行预热
	#[default]
	BlockNumber,
	/// 使用 eth_chainId 方法进行预热
	ChainId,
	/// 使用 net_version 方法进行预热
	NetVersion,
}

/// 智能 Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialFailoverConfig {
	/// 分数计算中稳定性的权重 (0.0-1.0)
	#[serde(default = "default_stability_weight")]
	pub stability_weight: f64,

	/// 分数计算中延迟的权重 (0.0-1.0)
	#[serde(default = "default_latency_weight")]
	pub latency_weight: f64,

	/// 分数计算中健康检查数据的权重 (0.0-1.0)
	#[serde(default = "default_health_weight")]
	pub health_weight: f64,

	/// 分数计算中历史表现的权重 (0.0-1.0)
	#[serde(default = "default_history_weight")]
	pub history_weight: f64,

	/// 滑动窗口的样本数量
	#[serde(default = "default_sample_count")]
	pub sample_count: usize,

	/// 连续失败多少次后认为节点不健康
	#[serde(default = "default_unhealthy_threshold")]
	pub unhealthy_threshold: u32,

	/// 连续失败多少次后开始分数惩罚
	#[serde(default = "default_penalty_threshold")]
	pub penalty_threshold: u32,

	/// 分数惩罚系数 (0.0-1.0)
	#[serde(default = "default_penalty_factor")]
	pub penalty_factor: f64,

	/// 排名日志记录间隔
	#[serde(default = "default_ranking_log_interval")]
	pub ranking_log_interval_secs: u64,

	/// 是否启用详细日志
	#[serde(default = "default_enable_verbose_logging")]
	pub enable_verbose_logging: bool,

	/// 是否优先使用最佳节点（延迟最低），而不是轮询负载均衡
	#[serde(default = "default_prefer_best_node")]
	pub prefer_best_node: bool,

	/// 节点冷却时间（秒）- 连续失败节点的暂时排除时间
	#[serde(default = "default_node_cooldown_secs")]
	pub node_cooldown_secs: u64,

	/// 是否启用节点预热
	#[serde(default = "default_enable_warmup")]
	pub enable_warmup: bool,

	/// 预热请求次数
	#[serde(default = "default_warmup_requests")]
	pub warmup_requests: u32,

	/// 预热超时时间（秒）
	#[serde(default = "default_warmup_timeout_secs")]
	pub warmup_timeout_secs: u64,

	/// 预热方法
	#[serde(default)]
	pub warmup_method: WarmupMethod,

	/// 多维度评分配置
	#[serde(default)]
	pub scoring: ScoringConfig,
}

impl Default for SerialFailoverConfig {
	fn default() -> Self {
		Self {
			stability_weight: default_stability_weight(),
			latency_weight: default_latency_weight(),
			health_weight: default_health_weight(),
			history_weight: default_history_weight(),
			sample_count: default_sample_count(),
			unhealthy_threshold: default_unhealthy_threshold(),
			penalty_threshold: default_penalty_threshold(),
			penalty_factor: default_penalty_factor(),
			ranking_log_interval_secs: default_ranking_log_interval(),
			enable_verbose_logging: default_enable_verbose_logging(),
			prefer_best_node: default_prefer_best_node(),
			node_cooldown_secs: default_node_cooldown_secs(),
			enable_warmup: default_enable_warmup(),
			warmup_requests: default_warmup_requests(),
			warmup_timeout_secs: default_warmup_timeout_secs(),
			warmup_method: WarmupMethod::default(),
			scoring: ScoringConfig::default(),
		}
	}
}

impl SerialFailoverConfig {
	/// 验证配置的有效性
	pub fn validate(&self) -> AppResult<()> {
		if self.stability_weight < 0.0 || self.stability_weight > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"stability_weight must be between 0.0 and 1.0"
			);
		}

		if self.latency_weight < 0.0 || self.latency_weight > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"latency_weight must be between 0.0 and 1.0"
			);
		}

		if self.health_weight < 0.0 || self.health_weight > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"health_weight must be between 0.0 and 1.0"
			);
		}

		if self.history_weight < 0.0 || self.history_weight > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"history_weight must be between 0.0 and 1.0"
			);
		}

		// 验证所有权重之和为1.0
		let total_weight = self.stability_weight + self.latency_weight + self.health_weight + self.history_weight;
		if (total_weight - 1.0).abs() > 0.001 {
			return err!(AyeErr::FailoverConfigInvalid, "All weights must sum to 1.0");
		}

		if self.sample_count == 0 {
			return err!(AyeErr::FailoverConfigInvalid, "sample_count must be greater than 0");
		}

		if self.penalty_factor < 0.0 || self.penalty_factor > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"penalty_factor must be between 0.0 and 1.0"
			);
		}

		// 验证评分配置
		// match &self.scoring.algorithm {
		// 	ScoringAlgorithm::ExponentialMovingAverage { alpha } => {
		// 		if *alpha < 0.0 || *alpha > 1.0 {
		// 			return err!(
		// 				AyeErr::FailoverConfigInvalid,
		// 				"ExponentialMovingAverage alpha must be between 0.0 and 1.0"
		// 			);
		// 		}
		// 	}
		// 	_ => {}
		// }
		if let ScoringAlgorithm::ExponentialMovingAverage { alpha } = &self.scoring.algorithm
			&& (*alpha < 0.0 || *alpha > 1.0)
		{
			return err!(
				AyeErr::FailoverConfigInvalid,
				"ExponentialMovingAverage alpha must be between 0.0 and 1.0"
			);
		}

		if self.scoring.score_decay.factor < 0.0 || self.scoring.score_decay.factor > 1.0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"Score decay factor must be between 0.0 and 1.0"
			);
		}

		Ok(())
	}

	/// 获取排名日志记录间隔
	pub fn ranking_log_interval(&self) -> Duration {
		Duration::from_secs(self.ranking_log_interval_secs)
	}

	/// 获取节点冷却时间
	pub fn node_cooldown_duration(&self) -> Duration {
		Duration::from_secs(self.node_cooldown_secs)
	}

	/// 获取预热超时时间
	pub fn warmup_timeout_duration(&self) -> Duration {
		Duration::from_secs(self.warmup_timeout_secs)
	}
}

// 默认值函数
fn default_stability_weight() -> f64 {
	0.4
}
fn default_latency_weight() -> f64 {
	0.3
}
fn default_health_weight() -> f64 {
	0.2
}
fn default_history_weight() -> f64 {
	0.1
}
fn default_sample_count() -> usize {
	10
}
fn default_unhealthy_threshold() -> u32 {
	50
}
fn default_penalty_threshold() -> u32 {
	30
}
fn default_penalty_factor() -> f64 {
	0.5
}
fn default_ranking_log_interval() -> u64 {
	10
}
fn default_enable_verbose_logging() -> bool {
	false
}
fn default_prefer_best_node() -> bool {
	true
}
fn default_node_cooldown_secs() -> u64 {
	30
}
fn default_enable_warmup() -> bool {
	true
}
fn default_warmup_requests() -> u32 {
	1
}
fn default_warmup_timeout_secs() -> u64 {
	5
}

// 多维度评分相关默认值
fn default_adaptive_weights() -> bool {
	false
}
fn default_stabilization_period() -> u64 {
	300
} // 5分钟
fn default_latency_threshold() -> u64 {
	1000
} // 1秒
fn default_latency_penalty_factor() -> f64 {
	0.8
}
fn default_enable_latency_tiers() -> bool {
	true
}
fn default_enable_decay() -> bool {
	false
}
fn default_decay_factor() -> f64 {
	0.95
}
fn default_decay_interval() -> u64 {
	3600
} // 1小时

/// 传输节点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
	/// 节点唯一标识
	pub id: String,

	/// 节点URL
	pub url: String,

	/// 节点权重 (用于初始排名)
	#[serde(default = "default_weight")]
	pub weight: f64,

	/// 是否启用该节点
	#[serde(default = "default_enabled")]
	pub enabled: bool,

	/// 节点标签 (用于分类和过滤)
	#[serde(default)]
	pub tags: Vec<String>,

	/// 最大超时时间 (秒)
	#[serde(default = "default_timeout_secs")]
	pub timeout_secs: u64,
}

impl TransportConfig {
	/// 获取超时时间
	pub fn timeout(&self) -> Duration {
		Duration::from_secs(self.timeout_secs)
	}
}

fn default_weight() -> f64 {
	1.0
}
fn default_enabled() -> bool {
	true
}
fn default_timeout_secs() -> u64 {
	30
}

/// 健康检查探测方法
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProbeMethod {
	/// 使用 eth_blockNumber 查询
	BlockNumber,
	/// 使用 eth_getBalance 查询（推荐）
	GetBalance {
		/// 要检查的地址，默认为零地址
		address: String,
	},
	/// 使用 net_version 查询
	NetVersion,
	/// 使用 web3_clientVersion 查询
	ClientVersion,
	/// 自定义 RPC 方法
	Custom {
		/// 方法名
		method: String,
		/// 参数
		params: serde_json::Value,
	},
}

impl Default for ProbeMethod {
	fn default() -> Self {
		Self::GetBalance {
			address: "0x0000000000000000000000000000000000000000".to_string(),
		}
	}
}

/// 性能基准测试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarkConfig {
	/// 是否启用性能基准测试
	#[serde(default = "default_benchmark_enabled")]
	pub enabled: bool,

	/// 基准测试间隔（秒）
	#[serde(default = "default_benchmark_interval")]
	pub interval_secs: u64,

	/// 每次基准测试的请求数量
	#[serde(default = "default_benchmark_test_count")]
	pub test_count: u32,

	/// 基准测试超时时间（秒）
	#[serde(default = "default_benchmark_timeout")]
	pub timeout_secs: u64,

	/// 基准测试方法
	#[serde(default)]
	pub test_method: ProbeMethod,

	/// 是否在启动时执行一次基准测试
	#[serde(default = "default_benchmark_startup_test")]
	pub startup_test: bool,

	/// 是否启用基准测试日志
	#[serde(default = "default_benchmark_logging")]
	pub enable_logging: bool,

	/// 并发测试的最大节点数
	#[serde(default = "default_benchmark_max_concurrent")]
	pub max_concurrent_nodes: usize,

	/// 是否在基准测试后自动重新排序节点
	#[serde(default = "default_benchmark_auto_reorder")]
	pub auto_reorder_after_test: bool,
}

impl Default for PerformanceBenchmarkConfig {
	fn default() -> Self {
		Self {
			enabled: default_benchmark_enabled(),
			interval_secs: default_benchmark_interval(),
			test_count: default_benchmark_test_count(),
			timeout_secs: default_benchmark_timeout(),
			test_method: ProbeMethod::default(),
			startup_test: default_benchmark_startup_test(),
			enable_logging: default_benchmark_logging(),
			max_concurrent_nodes: default_benchmark_max_concurrent(),
			auto_reorder_after_test: default_benchmark_auto_reorder(),
		}
	}
}

impl PerformanceBenchmarkConfig {
	/// 获取基准测试间隔
	pub fn interval(&self) -> Duration {
		Duration::from_secs(self.interval_secs)
	}

	/// 获取基准测试超时时间
	pub fn timeout(&self) -> Duration {
		Duration::from_secs(self.timeout_secs)
	}

	/// 验证基准测试配置
	pub fn validate(&self) -> AppResult<()> {
		if self.interval_secs == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"benchmark interval must be greater than 0"
			);
		}

		if self.timeout_secs == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"benchmark timeout must be greater than 0"
			);
		}

		if self.test_count == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"benchmark test count must be greater than 0"
			);
		}

		if self.max_concurrent_nodes == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"max concurrent nodes must be greater than 0"
			);
		}

		// 基准测试的超时应该合理，不能太长影响整体间隔
		if self.timeout_secs * self.test_count as u64 > self.interval_secs / 2 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"benchmark total timeout is too long compared to interval"
			);
		}

		Ok(())
	}
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
	/// 是否启用健康检查
	#[serde(default = "default_health_check_enabled")]
	pub enabled: bool,

	/// 健康检查间隔（秒）
	#[serde(default = "default_health_check_interval")]
	pub interval_secs: u64,

	/// 健康检查超时时间（秒）
	#[serde(default = "default_health_check_timeout")]
	pub timeout_secs: u64,

	/// 探测方法
	#[serde(default)]
	pub probe_method: ProbeMethod,

	/// 连续成功多少次后认为节点恢复健康
	#[serde(default = "default_recovery_threshold")]
	pub recovery_threshold: u32,

	/// 连续失败多少次后认为节点不健康
	#[serde(default = "default_failure_threshold")]
	pub failure_threshold: u32,

	/// 是否在启动时执行一次健康检查
	#[serde(default = "default_startup_check")]
	pub startup_check: bool,

	/// 是否启用健康检查日志
	#[serde(default = "default_health_check_logging")]
	pub enable_logging: bool,

	/// 性能基准测试配置
	#[serde(default)]
	pub performance_benchmark: PerformanceBenchmarkConfig,
}

impl Default for HealthCheckConfig {
	fn default() -> Self {
		Self {
			enabled: default_health_check_enabled(),
			interval_secs: default_health_check_interval(),
			timeout_secs: default_health_check_timeout(),
			probe_method: ProbeMethod::default(),
			recovery_threshold: default_recovery_threshold(),
			failure_threshold: default_failure_threshold(),
			startup_check: default_startup_check(),
			enable_logging: default_health_check_logging(),
			performance_benchmark: PerformanceBenchmarkConfig::default(),
		}
	}
}

impl HealthCheckConfig {
	/// 获取健康检查间隔
	pub fn interval(&self) -> Duration {
		Duration::from_secs(self.interval_secs)
	}

	/// 获取健康检查超时时间
	pub fn timeout(&self) -> Duration {
		Duration::from_secs(self.timeout_secs)
	}

	/// 验证健康检查配置
	pub fn validate(&self) -> AppResult<()> {
		if self.interval_secs == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"health check interval must be greater than 0"
			);
		}

		if self.timeout_secs == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"health check timeout must be greater than 0"
			);
		}

		if self.timeout_secs >= self.interval_secs {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"health check timeout must be less than interval"
			);
		}

		if self.recovery_threshold == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"recovery threshold must be greater than 0"
			);
		}

		if self.failure_threshold == 0 {
			return err!(
				AyeErr::FailoverConfigInvalid,
				"failure threshold must be greater than 0"
			);
		}

		// 验证性能基准测试配置
		if self.performance_benchmark.enabled {
			self.performance_benchmark.validate()?;
		}

		Ok(())
	}
}

// 健康检查默认值函数
fn default_health_check_enabled() -> bool {
	true
}
fn default_health_check_interval() -> u64 {
	60
} // 1分钟
fn default_health_check_timeout() -> u64 {
	5
} // 5秒
fn default_recovery_threshold() -> u32 {
	3
}
fn default_failure_threshold() -> u32 {
	3
}
fn default_startup_check() -> bool {
	true
}
fn default_health_check_logging() -> bool {
	true
}

// 性能基准测试默认值函数
fn default_benchmark_enabled() -> bool {
	true
}
fn default_benchmark_interval() -> u64 {
	300
} // 5分钟
fn default_benchmark_test_count() -> u32 {
	5
}
fn default_benchmark_timeout() -> u64 {
	10
} // 10秒
fn default_benchmark_startup_test() -> bool {
	false
}
fn default_benchmark_logging() -> bool {
	true
}
fn default_benchmark_max_concurrent() -> usize {
	10
}
fn default_benchmark_auto_reorder() -> bool {
	true
}

/// 完整的智能 Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSerialFallbackConfig {
	/// 智能 Provider 全局配置
	#[serde(default)]
	pub provider: SerialFailoverConfig,

	/// 健康检查配置
	#[serde(default)]
	pub health_check: HealthCheckConfig,

	/// 传输节点配置列表
	pub transports: Vec<TransportConfig>,
}

impl FullSerialFallbackConfig {
	/// 从 YAML 文件加载配置
	pub fn from_yaml_file(path: &str) -> AppResult<Self> {
		let content = std::fs::read_to_string(path).map_err(map_err!(AyeErr::FailoverFileOperationFailed))?;
		let config: Self = serde_yaml::from_str(&content).map_err(map_err!(AyeErr::FailoverSerializationFailed))?;
		config.provider.validate()?;
		config.health_check.validate()?;
		Ok(config)
	}

	/// 从 JSON 文件加载配置
	pub fn from_json_file(path: &str) -> AppResult<Self> {
		let content = std::fs::read_to_string(path).map_err(map_err!(AyeErr::FailoverFileOperationFailed))?;
		let config: Self = serde_json::from_str(&content).map_err(map_err!(AyeErr::FailoverSerializationFailed))?;
		config.provider.validate()?;
		config.health_check.validate()?;
		Ok(config)
	}

	/// 保存配置到 YAML 文件
	pub fn save_to_yaml_file(&self, path: &str) -> AppResult<()> {
		let content = serde_yaml::to_string(self).map_err(map_err!(AyeErr::FailoverSerializationFailed))?;
		std::fs::write(path, content).map_err(map_err!(AyeErr::FailoverFileOperationFailed))?;
		Ok(())
	}

	/// 保存配置到 JSON 文件
	pub fn save_to_json_file(&self, path: &str) -> AppResult<()> {
		let content = serde_json::to_string_pretty(self).map_err(map_err!(AyeErr::FailoverSerializationFailed))?;
		std::fs::write(path, content).map_err(map_err!(AyeErr::FailoverFileOperationFailed))?;
		Ok(())
	}

	/// 获取启用的传输节点配置
	pub fn enabled_transports(&self) -> Vec<&TransportConfig> {
		self.transports.iter().filter(|t| t.enabled).collect()
	}

	/// 根据标签过滤传输节点
	pub fn transports_with_tag(&self, tag: &str) -> Vec<&TransportConfig> {
		self.transports
			.iter()
			.filter(|t| t.enabled && t.tags.contains(&tag.to_string()))
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_smart_provider_config_default() {
		let config = SerialFailoverConfig::default();
		assert_eq!(config.stability_weight, 0.4);
		assert_eq!(config.latency_weight, 0.3);
		assert_eq!(config.health_weight, 0.2);
		assert_eq!(config.history_weight, 0.1);
		assert_eq!(config.sample_count, 10);
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_smart_provider_config_validation() {
		let mut config = SerialFailoverConfig::default();

		// 测试无效的稳定性权重
		config.stability_weight = 1.5;
		assert!(config.validate().is_err());

		// 重置为有效值
		config.stability_weight = 0.4;
		config.latency_weight = 0.3;
		config.health_weight = 0.2;
		config.history_weight = 0.2; // 总和不等于1.0
		assert!(config.validate().is_err());

		// 修正权重
		config.history_weight = 0.1;
		config.sample_count = 0;
		assert!(config.validate().is_err());

		// 修正样本数
		config.sample_count = 10;
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_scoring_config_default() {
		let config = ScoringConfig::default();
		assert!(!config.adaptive_weights);
		assert_eq!(config.stabilization_period_secs, 300);
		assert_eq!(config.latency_sensitivity.threshold_ms, 1000);
		assert!(!config.score_decay.enabled);

		match config.algorithm {
			ScoringAlgorithm::SimpleWeighted => {}
			_ => panic!("Expected SimpleWeighted algorithm"),
		}
	}

	#[test]
	fn test_scoring_algorithm_validation() {
		let mut config = SerialFailoverConfig::default();

		// 测试指数移动平均算法的alpha参数
		config.scoring.algorithm = ScoringAlgorithm::ExponentialMovingAverage { alpha: 1.5 };
		assert!(config.validate().is_err());

		config.scoring.algorithm = ScoringAlgorithm::ExponentialMovingAverage { alpha: 0.8 };
		assert!(config.validate().is_ok());

		// 测试分数衰减因子
		config.scoring.score_decay.factor = 1.5;
		assert!(config.validate().is_err());

		config.scoring.score_decay.factor = 0.95;
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_transport_config() {
		let config = TransportConfig {
			id: "test-node".to_string(),
			url: "https://eth.llamarpc.com".to_string(),
			weight: 1.0,
			enabled: true,
			tags: vec!["mainnet".to_string(), "public".to_string()],
			timeout_secs: 30,
		};

		assert_eq!(config.timeout(), Duration::from_secs(30));
	}

	#[test]
	fn test_full_config_filtering() {
		let config = FullSerialFallbackConfig {
			provider: SerialFailoverConfig::default(),
			health_check: HealthCheckConfig::default(),
			transports: vec![
				TransportConfig {
					id: "node1".to_string(),
					url: "https://node1.com".to_string(),
					weight: 1.0,
					enabled: true,
					tags: vec!["mainnet".to_string()],
					timeout_secs: 30,
				},
				TransportConfig {
					id: "node2".to_string(),
					url: "https://node2.com".to_string(),
					weight: 1.0,
					enabled: false,
					tags: vec!["testnet".to_string()],
					timeout_secs: 30,
				},
				TransportConfig {
					id: "node3".to_string(),
					url: "https://node3.com".to_string(),
					weight: 1.0,
					enabled: true,
					tags: vec!["mainnet".to_string(), "premium".to_string()],
					timeout_secs: 30,
				},
			],
		};

		let enabled = config.enabled_transports();
		assert_eq!(enabled.len(), 2);

		let mainnet_nodes = config.transports_with_tag("mainnet");
		assert_eq!(mainnet_nodes.len(), 2);

		let premium_nodes = config.transports_with_tag("premium");
		assert_eq!(premium_nodes.len(), 1);
	}

	#[test]
	fn test_config_serialization() {
		let config = FullSerialFallbackConfig {
			provider: SerialFailoverConfig::default(),
			health_check: HealthCheckConfig::default(),
			transports: vec![TransportConfig {
				id: "test-node".to_string(),
				url: "https://eth.llamarpc.com".to_string(),
				weight: 1.0,
				enabled: true,
				tags: vec!["mainnet".to_string()],
				timeout_secs: 30,
			}],
		};

		// 测试 JSON 序列化
		let json_str = serde_json::to_string(&config).unwrap();
		let deserialized: FullSerialFallbackConfig = serde_json::from_str(&json_str).unwrap();
		assert_eq!(deserialized.transports.len(), 1);

		// 测试 YAML 序列化
		let yaml_str = serde_yaml::to_string(&config).unwrap();
		let deserialized: FullSerialFallbackConfig = serde_yaml::from_str(&yaml_str).unwrap();
		assert_eq!(deserialized.transports.len(), 1);
	}

	#[test]
	fn test_health_check_config_default() {
		let config = HealthCheckConfig::default();
		assert!(config.enabled);
		assert_eq!(config.interval_secs, 60);
		assert_eq!(config.timeout_secs, 5);
		assert_eq!(config.recovery_threshold, 3);
		assert_eq!(config.failure_threshold, 3);
		assert!(config.startup_check);
		assert!(config.enable_logging);
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_health_check_config_validation() {
		let mut config = HealthCheckConfig::default();

		// 测试无效的间隔
		config.interval_secs = 0;
		assert!(config.validate().is_err());

		config.interval_secs = 60;
		config.timeout_secs = 0;
		assert!(config.validate().is_err());

		// 测试超时时间大于等于间隔时间
		config.timeout_secs = 60;
		assert!(config.validate().is_err());

		config.timeout_secs = 70;
		assert!(config.validate().is_err());

		// 测试阈值为0
		config.timeout_secs = 5;
		config.recovery_threshold = 0;
		assert!(config.validate().is_err());

		config.recovery_threshold = 3;
		config.failure_threshold = 0;
		assert!(config.validate().is_err());

		// 恢复正常配置
		config.failure_threshold = 3;
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_probe_method_serialization() {
		let methods = vec![
			ProbeMethod::BlockNumber,
			ProbeMethod::GetBalance {
				address: "0x1234567890123456789012345678901234567890".to_string(),
			},
			ProbeMethod::NetVersion,
			ProbeMethod::ClientVersion,
			ProbeMethod::Custom {
				method: "custom_method".to_string(),
				params: serde_json::json!({"key": "value"}),
			},
		];

		for method in methods {
			let json_str = serde_json::to_string(&method).unwrap();
			let deserialized: ProbeMethod = serde_json::from_str(&json_str).unwrap();

			match (&method, &deserialized) {
				(ProbeMethod::BlockNumber, ProbeMethod::BlockNumber) => {}
				(ProbeMethod::GetBalance { address: a1 }, ProbeMethod::GetBalance { address: a2 }) => {
					assert_eq!(a1, a2);
				}
				(ProbeMethod::NetVersion, ProbeMethod::NetVersion) => {}
				(ProbeMethod::ClientVersion, ProbeMethod::ClientVersion) => {}
				(ProbeMethod::Custom { method: m1, params: p1 }, ProbeMethod::Custom { method: m2, params: p2 }) => {
					assert_eq!(m1, m2);
					assert_eq!(p1, p2);
				}
				_ => panic!("Serialization/deserialization mismatch"),
			}
		}
	}

	#[test]
	fn test_performance_benchmark_config_default() {
		let config = PerformanceBenchmarkConfig::default();
		assert!(config.enabled);
		assert_eq!(config.interval_secs, 300);
		assert_eq!(config.test_count, 5);
		assert_eq!(config.timeout_secs, 10);
		assert!(!config.startup_test);
		assert!(config.enable_logging);
		assert_eq!(config.max_concurrent_nodes, 10);
		assert!(config.auto_reorder_after_test);
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_performance_benchmark_config_validation() {
		let mut config = PerformanceBenchmarkConfig::default();

		// 测试无效的间隔
		config.interval_secs = 0;
		assert!(config.validate().is_err());

		config.interval_secs = 300;
		config.timeout_secs = 0;
		assert!(config.validate().is_err());

		config.timeout_secs = 10;
		config.test_count = 0;
		assert!(config.validate().is_err());

		config.test_count = 5;
		config.max_concurrent_nodes = 0;
		assert!(config.validate().is_err());

		// 测试超时时间过长
		config.max_concurrent_nodes = 10;
		config.timeout_secs = 100;
		config.test_count = 10;
		config.interval_secs = 300;
		assert!(config.validate().is_err()); // 100 * 10 > 300 / 2

		// 恢复正常配置
		config.timeout_secs = 10;
		config.test_count = 5;
		assert!(config.validate().is_ok());
	}

	#[test]
	fn test_health_check_with_benchmark_config() {
		let mut health_config = HealthCheckConfig::default();

		// 默认配置应该包含基准测试配置
		assert!(health_config.performance_benchmark.enabled);
		assert!(health_config.validate().is_ok());

		// 测试禁用基准测试后验证仍然有效
		health_config.performance_benchmark.enabled = false;
		assert!(health_config.validate().is_ok());

		// 测试启用基准测试但配置无效
		health_config.performance_benchmark.enabled = true;
		health_config.performance_benchmark.interval_secs = 0;
		assert!(health_config.validate().is_err());

		// 修复配置
		health_config.performance_benchmark.interval_secs = 300;
		assert!(health_config.validate().is_ok());
	}
}
