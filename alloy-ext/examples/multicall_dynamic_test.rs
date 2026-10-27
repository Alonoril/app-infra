use alloy::network::Network;
use alloy::primitives::{U256, address};
use alloy::providers::{CallInfoTrait, CallTuple, MulticallBuilder, MulticallItem, Provider, ProviderBuilder};
use alloy::sol_types::SolCall;

alloy::sol! {
   #[sol(rpc)]
   #[derive(Debug, PartialEq)]
   interface ERC20 {
	   function totalSupply() external view returns (uint256 totalSupply);
	   function balanceOf(address owner) external view returns (uint256 balance);
   }
}
#[tokio::main]
async fn main() {
	let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
	let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

	let provider = ProviderBuilder::new().connect_http("https://eth.merkle.io".parse().unwrap());
	let weth = ERC20::new(weth, &provider);
	let usdc = ERC20::new(usdc, &provider);

	let owner = address!("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");

	let mut erc20_ins = MulticallBuilder::new_dynamic(&provider);

	for token in &[weth, usdc] {
		erc20_ins = erc20_ins.add_dynamic(token.balanceOf(owner)); //.add_dynamic(token.totalSupply());
	}

	let balances: Vec<U256> = erc20_ins.aggregate().await.unwrap();

	let weth_bal = &balances[0];
	let usdc_bal = &balances[1];
	println!("WETH Balance: {:?}, USDC Balance: {:?}", weth_bal, usdc_bal);
}

// #[tokio::main]
// async fn main() {
//     let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
//     let provider = ProviderBuilder::new().on_http("https:// eth. merkle. io".parse().unwrap());
//     let erc20 = ERC20::new(weth, &provider);
//
//     let ts_call = erc20.totalSupply();
//     let balance_call = erc20.balanceOf(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
//
//     let mut calls: Vec<Call3Value> = Vec::new();
//
//     let call_val = build_call3_value(ts_call);
//     calls.push(call_val);
//     let call_val = build_call3_value(balance_call);
//     calls.push(call_val);
//
//     let multicall = provider.multicall();
//
//
//     let (total_supply, balance) = multicall.aggregate().await.unwrap();
//
//     println!("Total Supply: {:?}, Balance: {:?}", total_supply, balance);
// }

// pub trait MulticallExt<T: CallTuple, P: Provider<N>, N: Network> {
//     fn add_calls(self, calls: Vec<Call3Value>) -> MulticallBuilder<T::Pushed, P, N>;
// }
// impl<T, P, N> MulticallExt<T, P, N> for MulticallBuilder<T, &P, N>
// where
//     T: CallTuple,
// 	T: TuplePush<Item::Decoder>,
//     P: Provider<N> + Clone,
//     N: Network,
// {
//     fn add_calls(mut self, calls: Vec<Call3Value>) -> MulticallBuilder<T::Pushed, P, N> {
// 		MulticallBuilder {
// 			calls: self.calls,
// 			provider: self.provider,
// 			block: self.block,
// 			state_override: self.state_override,
// 			address: self.address,
// 			input_kind: self.input_kind,
// 			_pd: Default::default(),
// 		}
//     }
// }
//
// pub fn build_call3_value<T: CallTuple, Item: MulticallItem>(item: Item) -> Call3Value
// where
//     Item::Decoder: 'static,
// // T: TuplePush<Item::Decoder>,
// // <T as TuplePush<Item::Decoder>>::Pushed: CallTuple,
// {
//     let target = item.target();
//     let input = item.input();
//
//     let call = CallItem::<Item::Decoder>::new(target, input);
//
//     call.to_call3_value()
// }
