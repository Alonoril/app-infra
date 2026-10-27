// use crate::UniV3Pool::factoryReturn;
// use alloy::network::Network;
// use alloy::primitives::{Address, U160, U256, address};
// use alloy::providers::{CallInfoTrait, CallTuple, MulticallBuilder, MulticallItem, Provider, ProviderBuilder};
// use alloy::sol_types::SolCall;
//
// alloy::sol!(
// 	#[allow(missing_docs)]
// 	#[sol(rpc)]
// 	#[derive(Debug)]
// 	UniV3Pool,
// 	"abis/UniswapV3Pool.json"
// );
//
// #[tokio::main]
// async fn main() {
// 	let weth = address!("0x109830a1AAaD605BbF02a9dFA7B0B92EC2FB7dAa");
// 	let usdc = address!("0xD91c0224c17098A3bB8B3f5404a56BD1306eEA98");
//
// 	let provider = ProviderBuilder::new().connect_http("https://eth.merkle.io".parse().unwrap());
// 	let weth = UniV3Pool::new(weth, &provider);
// 	let usdc = UniV3Pool::new(usdc, &provider);
//
// 	let mut erc20_ins = MulticallBuilder::new_dynamic(&provider);
//
// 	for token in &[weth, usdc] {
// 		erc20_ins = erc20_ins.add_dynamic(token.factory()); //.add_dynamic(token.token0()).add_dynamic(token.token1());
// 	}
//
// 	let vals: Vec<Address> = erc20_ins.aggregate().await.unwrap();
// 	// for val in vals {
// 	// 	println!("{:?}", val);
// 	// }
// }
//
// // #[tokio::main]
// // async fn main() {
// //     let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
// //     let provider = ProviderBuilder::new().on_http("https:// eth. merkle. io".parse().unwrap());
// //     let erc20 = ERC20::new(weth, &provider);
// //
// //     let ts_call = erc20.totalSupply();
// //     let balance_call = erc20.balanceOf(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
// //
// //     let mut calls: Vec<Call3Value> = Vec::new();
// //
// //     let call_val = build_call3_value(ts_call);
// //     calls.push(call_val);
// //     let call_val = build_call3_value(balance_call);
// //     calls.push(call_val);
// //
// //     let multicall = provider.multicall();
// //
// //
// //     let (total_supply, balance) = multicall.aggregate().await.unwrap();
// //
// //     println!("Total Supply: {:?}, Balance: {:?}", total_supply, balance);
// // }
//
// // pub trait MulticallExt<T: CallTuple, P: Provider<N>, N: Network> {
// //     fn add_calls(self, calls: Vec<Call3Value>) -> MulticallBuilder<T::Pushed, P, N>;
// // }
// // impl<T, P, N> MulticallExt<T, P, N> for MulticallBuilder<T, &P, N>
// // where
// //     T: CallTuple,
// // 	T: TuplePush<Item::Decoder>,
// //     P: Provider<N> + Clone,
// //     N: Network,
// // {
// //     fn add_calls(mut self, calls: Vec<Call3Value>) -> MulticallBuilder<T::Pushed, P, N> {
// // 		MulticallBuilder {
// // 			calls: self.calls,
// // 			provider: self.provider,
// // 			block: self.block,
// // 			state_override: self.state_override,
// // 			address: self.address,
// // 			input_kind: self.input_kind,
// // 			_pd: Default::default(),
// // 		}
// //     }
// // }
// //
// // pub fn build_call3_value<T: CallTuple, Item: MulticallItem>(item: Item) -> Call3Value
// // where
// //     Item::Decoder: 'static,
// // // T: TuplePush<Item::Decoder>,
// // // <T as TuplePush<Item::Decoder>>::Pushed: CallTuple,
// // {
// //     let target = item.target();
// //     let input = item.input();
// //
// //     let call = CallItem::<Item::Decoder>::new(target, input);
// //
// //     call.to_call3_value()
// // }

fn main() {}
