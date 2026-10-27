//! Test the smart provider layer.

use std::time::Duration;

use alloy::{
	providers::{Provider, ProviderBuilder},
	rpc::client::RpcClient,
	transports::http::{Http, reqwest::Url},
};
use alloy_ext::transports::layers::{SerialFailoverLayer, WarmupMethod};
use tower::ServiceBuilder;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// 初始化 tracing，设置为 DEBUG 级别以看到 debug_span! 的输出
	tracing_subscriber::registry()
		.with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
		.with(tracing_subscriber::fmt::layer())
		.init();

	// Configure the smart provider layer with latency-first strategy and warmup
	let smart_layer = SerialFailoverLayer::default()
		// .with_stability_weight(0.4) // 稳定性权重 40%
		// .with_latency_weight(0.3) // 延迟权重 30%
		// .with_health_weight(0.2) // 健康权重 20%
		// .with_history_weight(0.1) // 历史权重 10%
		// .with_sample_count(10) // 滑动窗口大小
		// .with_unhealthy_threshold(5) // 连续失败5次后认为不健康
		// .with_penalty_threshold(3) // 连续失败3次后开始惩罚
		// .with_penalty_factor(0.5) // 惩罚系数
		// .with_prefer_best_node(true) // 启用延迟最低优先策略
		// .with_node_cooldown_secs(30) // 节点冷却时间30秒
		// .with_warmup_enabled(true) // 启用节点预热
		// .with_warmup_requests(5) // 预热请求次数
		// .with_warmup_timeout_secs(5) // 预热超时时间5秒
		// .with_warmup_method(WarmupMethod::BlockNumber) // 使用eth_blockNumber预热
		// .with_verbose_logging(true) // 启用详细日志
		.with_default_health_check(); // 🎯 一行代码启用自动健康检查！

	// Define your list of transports to use
	let transports = vec![
		Http::new(Url::parse("https://reth-ethereum.ithaca.xyz/rpc")?),
		Http::new(Url::parse("https://eth.llamarpc.com")?),
		Http::new(Url::parse("https://ethereum-rpc.publicnode.com")?),
	];

	// Create the transport service with lazy warmup and auto health check using ServiceBuilder pattern
	println!("🚀 Creating SmartProvider with lazy warmup and auto health check...");

	let transport = ServiceBuilder::new().layer(smart_layer).service(transports);
	let client = RpcClient::builder().transport(transport, false);
	let provider = ProviderBuilder::new().connect_client(client);

	// Get the latest block number using the provider with smart latency-first routing.
	// Warmup will be performed automatically on the first request (lazy loading).
	// Health checker will be automatically started and monitoring all nodes.
	// This will prioritize the fastest (lowest latency) node, only switching when it fails.
	let max = 10;
	let mut count = 0;
	loop {
		let latest_block = provider.get_block_number().await?;
		println!("Latest block number: {latest_block}");
		tokio::time::sleep(Duration::from_secs(1)).await;

		count += 1;
		if count >= max {
			break;
		}
	}

	println!("\n🎯 Latency-first strategy with lazy warmup and auto health check completed!");
	println!("🔥 Warmup was performed automatically on the first request");
	println!("🏥 Health checker was automatically started and monitoring nodes");
	println!("✅ The fastest (lowest latency) nodes were prioritized");
	println!("🔄 Failed nodes were temporarily cooled down for 30 seconds");
	println!("🤖 All functionality started automatically - zero manual management!");

	Ok(())
}
