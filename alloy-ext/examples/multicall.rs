// use alloy::primitives::address;
// use alloy::providers::{Provider, ProviderBuilder};
// use alloy::sol_types::SolCall;
// use alloy_ext::provider::multicall::bindings::IMulticall3::Call3Value;
// use alloy_ext::provider::multicall::{build_call3_value, MulticallBuilder};
//
// alloy::sol! {
//    #[sol(rpc)]
//    #[derive(Debug, PartialEq)]
//    interface ERC20 {
// 	   function totalSupply() external view returns (uint256 totalSupply);
// 	   function balanceOf(address owner) external view returns (uint256 balance);
//    }
// }
//
// #[tokio::main]
// async fn main() {
// 	let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
// 	let provider = ProviderBuilder::new().on_http("https:// eth. merkle. io".parse().unwrap());
// 	let erc20 = ERC20::new(weth, &provider);
//
// 	let ts_call = erc20.totalSupply();
// 	let balance_call = erc20.balanceOf(address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
//
// 	let mut calls: Vec<Call3Value> = Vec::new();
//
// 	let call_val = build_call3_value(ts_call);
// 	calls.push(call_val);
// 	let call_val = build_call3_value(balance_call);
// 	calls.push(call_val);
//
// 	let multicall = MulticallBuilder::new(provider);
// 	let multicall = multicall.add_call3_values(calls);
//
// 	let ss = multicall.aggregate().await.unwrap();
//
// 	// println!("Total Supply: {:?}, Balance: {:?}", total_supply, balance);
// }

fn main() {}
