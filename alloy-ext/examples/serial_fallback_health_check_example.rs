//! 自动健康检查集成示例
//!
//! 这个示例展示了如何使用新的自动健康检查集成功能。
//! 现在你只需要在创建 SerialFallbackLayer 时配置健康检查，
//! 就能自动启动健康检查器，无需手动管理！

use std::time::Duration;

use alloy::{
	providers::{Provider, ProviderBuilder},
	rpc::client::RpcClient,
	transports::http::{Http, reqwest::Url},
};
use alloy_ext::transports::layers::{
	HealthCheckConfig, PerformanceBenchmarkConfig, ProbeMethod, SerialFailoverLayer, WarmupMethod,
};
use tower::ServiceBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// 设置日志
	tracing_subscriber::registry()
		.with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "debug".into()))
		.with(tracing_subscriber::fmt::layer())
		.init();

	println!("🏥 自动健康检查集成示例");
	println!("======================");

	// 1. 基础自动健康检查示例
	println!("📋 1. 基础自动健康检查");
	basic_auto_health_check_example().await?;

	println!("{}", format!("\n{}\n", "=".repeat(50).as_str()));

	// 2. 高级自动健康检查示例
	println!("🔬 2. 高级自动健康检查与性能测试");
	advanced_auto_health_check_example().await?;

	println!("{}", format!("\n{}\n", "=".repeat(50).as_str()));

	// 3. 完全自动化示例
	println!("⚡ 3. 完全自动化 - 一行配置搞定");
	fully_automated_example().await?;

	println!("\n✅ 所有自动健康检查示例已完成！");
	println!("🎯 总结：现在只需要一行配置就能启用自动健康检查，无需手动管理！");
	Ok(())
}

/// 基础自动健康检查示例
async fn basic_auto_health_check_example() -> anyhow::Result<()> {
	println!("🚀 创建基础自动健康检查配置...");

	// 创建健康检查配置
	let health_config = HealthCheckConfig {
		enabled: true,
		interval_secs: 10,                      // 每10秒检查一次
		timeout_secs: 5,                        // 5秒超时
		probe_method: ProbeMethod::BlockNumber, // 使用区块号探测
		recovery_threshold: 2,                  // 连续成功2次后恢复
		failure_threshold: 3,                   // 连续失败3次后标记为不健康
		startup_check: true,                    // 启动时检查
		enable_logging: true,                   // 启用日志
		performance_benchmark: PerformanceBenchmarkConfig::default(),
	};

	// 创建智能层并配置健康检查 - 这里是关键！
	let smart_layer = SerialFailoverLayer::default()
		.with_verbose_logging(true)
		.with_health_check(health_config); // 🎯 一行代码启用自动健康检查！

	// 定义测试节点
	let transports = vec![
		Http::new(Url::parse("https://eth.llamarpc.com")?),
		Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
		Http::new(Url::parse("https://reth-ethereum.ithaca.xyz/rpc")?),
	];

	println!("✅ 配置完成，健康检查器将自动启动...");

	// 创建提供者 - 健康检查器会自动启动！
	let transport = ServiceBuilder::new().layer(smart_layer).service(transports);
	let client = RpcClient::builder().transport(transport, false);
	let provider = ProviderBuilder::new().connect_client(client);

	// 执行几次请求来观察健康检查效果
	for i in 1..=5 {
		println!("📤 执行第 {} 次请求...", i);
		match provider.get_block_number().await {
			Ok(block_number) => {
				println!("✅ 成功获取区块号: {}", block_number);
			}
			Err(e) => {
				println!("❌ 请求失败: {}", e);
			}
		}

		tokio::time::sleep(Duration::from_secs(3)).await;
	}

	println!("📊 基础自动健康检查功能：");
	println!("   ✨ 自动启动健康检查器");
	println!("   🔄 故障自动转移");
	println!("   📈 健康状态实时监控");
	println!("   🎯 零手动管理");

	Ok(())
}

/// 高级自动健康检查示例
async fn advanced_auto_health_check_example() -> anyhow::Result<()> {
	println!("🔬 创建高级自动健康检查配置...");

	// 创建高级健康检查配置（包含性能基准测试）
	let health_config = HealthCheckConfig {
		enabled: true,
		interval_secs: 20,
		timeout_secs: 10,
		probe_method: ProbeMethod::BlockNumber,
		recovery_threshold: 2,
		failure_threshold: 3,
		startup_check: true,
		enable_logging: true,
		performance_benchmark: PerformanceBenchmarkConfig {
			enabled: true,
			interval_secs: 60, // 每分钟进行一次性能测试
			test_count: 5,     // 每次测试5个请求
			timeout_secs: 5,   // 测试超时5秒
			test_method: ProbeMethod::BlockNumber,
			startup_test: true, // 启动时进行测试
			enable_logging: true,
			max_concurrent_nodes: 3,       // 最多同时测试3个节点
			auto_reorder_after_test: true, // 测试后自动重排序
		},
	};

	// 创建智能层 - 一行配置即可启用所有高级功能！
	let smart_layer = SerialFailoverLayer::default()
		.with_verbose_logging(true)
		.with_warmup_enabled(true)
		.with_health_check(health_config); // 🎯 自动启动健康检查和性能测试！

	// 定义测试节点
	let transports = vec![
		Http::new(Url::parse("https://eth.llamarpc.com")?),
		Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
		Http::new(Url::parse("https://reth-ethereum.ithaca.xyz/rpc")?),
	];

	println!("🧪 启动带高级功能的提供者...");

	// 创建提供者 - 所有功能自动启动！
	let transport = ServiceBuilder::new().layer(smart_layer).service(transports);
	let client = RpcClient::builder().transport(transport, false);
	let provider = ProviderBuilder::new().connect_client(client);

	// 执行一系列请求来观察高级功能
	println!("📊 执行测试请求（观察预热、健康检查和性能测试）...");
	for i in 1..=8 {
		println!("📤 高级测试请求 #{}", i);
		match provider.get_block_number().await {
			Ok(block_number) => {
				println!("✅ 区块号: {} (节点已自动优化)", block_number);
			}
			Err(e) => {
				println!("❌ 请求失败: {}", e);
			}
		}

		// 较长间隔以便观察性能基准测试的效果
		tokio::time::sleep(Duration::from_secs(10)).await;
	}

	println!("📈 高级自动健康检查功能：");
	println!("   ⚡ 自动性能基准测试");
	println!("   🏆 自动节点排序优化");
	println!("   📊 详细统计信息自动收集");
	println!("   🔄 自动重新排序");
	println!("   🔥 预热功能自动集成");

	Ok(())
}

/// 完全自动化示例 - 最简单的用法
async fn fully_automated_example() -> anyhow::Result<()> {
	println!("⚡ 最简单的用法 - 默认配置即可！");

	// 🎯 最简单的用法：一行配置启用默认健康检查
	let smart_layer = SerialFailoverLayer::default()
		.with_default_health_check() // 🔥 一行代码启用所有默认健康检查功能！
		.with_warmup_enabled(true)
		.with_verbose_logging(true);

	let transports = vec![
		Http::new(Url::parse("https://eth.llamarpc.com")?),
		Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
	];

	println!("🚀 创建完全自动化的智能提供者...");

	// 一行创建 - 所有功能自动启用！
	let transport = ServiceBuilder::new().layer(smart_layer).service(transports);
	let client = RpcClient::builder().transport(transport, false);
	let provider = ProviderBuilder::new().connect_client(client);

	// 直接使用，无需任何手动管理
	println!("📱 直接使用，零配置管理：");
	for i in 1..=3 {
		println!("📤 自动化请求 #{}", i);
		let block_number = provider.get_block_number().await?;
		println!("✅ 区块号: {} (全自动优化)", block_number);
		tokio::time::sleep(Duration::from_secs(2)).await;
	}

	println!("⚡ 完全自动化功能：");
	println!("   🎯 零配置启用");
	println!("   🤖 全自动健康监控");
	println!("   🔄 智能故障转移");
	println!("   📈 性能自动优化");
	println!("   🛡️ 无需手动管理");

	Ok(())
}

/// 配置示例模块
pub mod config_examples {
	use super::*;

	/// 生产环境配置 - 稳定性优先
	pub fn production_health_config() -> HealthCheckConfig {
		HealthCheckConfig {
			enabled: true,
			interval_secs: 60, // 较长检查间隔
			timeout_secs: 15,  // 较长超时
			probe_method: ProbeMethod::BlockNumber,
			recovery_threshold: 3, // 更高恢复阈值
			failure_threshold: 5,  // 更高失败阈值
			startup_check: true,
			enable_logging: true,
			performance_benchmark: PerformanceBenchmarkConfig {
				enabled: true,
				interval_secs: 300, // 每5分钟测试一次
				test_count: 20,     // 更多测试样本
				timeout_secs: 10,
				test_method: ProbeMethod::BlockNumber,
				startup_test: true,
				enable_logging: false, // 减少日志噪音
				max_concurrent_nodes: 5,
				auto_reorder_after_test: true,
			},
		}
	}

	/// 开发环境配置 - 快速检测
	pub fn development_health_config() -> HealthCheckConfig {
		HealthCheckConfig {
			enabled: true,
			interval_secs: 10, // 快速检查
			timeout_secs: 5,   // 短超时
			probe_method: ProbeMethod::BlockNumber,
			recovery_threshold: 1, // 快速恢复
			failure_threshold: 2,  // 快速失败检测
			startup_check: true,
			enable_logging: true, // 详细日志
			performance_benchmark: PerformanceBenchmarkConfig {
				enabled: true,
				interval_secs: 60, // 频繁测试
				test_count: 3,     // 较少测试样本
				timeout_secs: 3,
				test_method: ProbeMethod::BlockNumber,
				startup_test: true,
				enable_logging: true,
				max_concurrent_nodes: 3,
				auto_reorder_after_test: true,
			},
		}
	}

	/// 高可用配置 - 严格监控
	pub fn high_availability_health_config() -> HealthCheckConfig {
		HealthCheckConfig {
			enabled: true,
			interval_secs: 5, // 非常频繁的检查
			timeout_secs: 3,  // 短超时，快速检测问题
			probe_method: ProbeMethod::BlockNumber,
			recovery_threshold: 2,
			failure_threshold: 2, // 低失败阈值
			startup_check: true,
			enable_logging: true,
			performance_benchmark: PerformanceBenchmarkConfig {
				enabled: true,
				interval_secs: 30, // 频繁性能测试
				test_count: 10,
				timeout_secs: 2,
				test_method: ProbeMethod::BlockNumber,
				startup_test: true,
				enable_logging: true,
				max_concurrent_nodes: 10,
				auto_reorder_after_test: true,
			},
		}
	}
}

/// 使用示例函数
pub async fn run_example_with_custom_config(health_config: HealthCheckConfig) -> anyhow::Result<()> {
	println!("🚀 使用自定义配置运行自动健康检查...");

	let smart_layer = SerialFailoverLayer::default()
		.with_verbose_logging(true)
		.with_health_check(health_config); // 🎯 自动集成！

	let transports = vec![
		Http::new(Url::parse("https://eth.llamarpc.com")?),
		Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
	];

	let transport = ServiceBuilder::new().layer(smart_layer).service(transports);
	let client = RpcClient::builder().transport(transport, false);
	let provider = ProviderBuilder::new().connect_client(client);

	// 执行测试请求
	for i in 1..=3 {
		println!("📤 自定义配置测试请求 #{}", i);
		let block_number = provider.get_block_number().await?;
		println!("✅ 区块号: {}", block_number);
		tokio::time::sleep(Duration::from_secs(2)).await;
	}

	Ok(())
}

/// 实际使用示例
#[allow(dead_code)]
async fn practical_usage_example() -> anyhow::Result<()> {
	println!("💼 实际使用场景示例");

	// 根据环境选择配置
	let health_config = if cfg!(debug_assertions) {
		config_examples::development_health_config()
	} else {
		config_examples::production_health_config()
	};

	// 运行示例
	run_example_with_custom_config(health_config).await?;

	Ok(())
}
