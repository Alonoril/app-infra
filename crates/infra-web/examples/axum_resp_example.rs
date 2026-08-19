use axum::{Router, routing::get};
use axum_resp_macro::resp_data;
use http::StatusCode;
use infra_core::{
	err,
	result::{AppError, AppResult},
};
use infra_web::{
	resp::{HttpStatusMode, WebErr, set_http_status_mode},
	status_err,
};
use serde::Serialize;
use std::{
	env,
	net::{Ipv4Addr, SocketAddr},
};
use tokio::net::TcpListener;

#[resp_data]
async fn ret_empty() -> AppResult<()> {
	empty().await
}

async fn empty() -> AppResult<()> {
	println!("empty response");
	Ok(())
}

#[derive(Debug, Serialize)]
struct User {
	name: String,
	age: u8,
}

async fn user_info() -> AppResult<Option<User>> {
	let user = Some(User {
		name: "Zimu".to_string(),
		age: 30,
	});
	Ok(user)
}

#[resp_data]
async fn get_user() -> AppResult<Option<User>> {
	user_info().await
}

#[resp_data]
async fn user_null() -> AppResult<Option<User>> {
	let user = None;
	Ok::<_, AppError>(user)
}

#[resp_data]
async fn user_err() -> AppResult<Option<User>> {
	err!(WebErr::SourceNotFound, "user not found")
}

#[resp_data]
async fn user_auth_err() -> AxumResult<Option<User>> {
	status_err!(StatusCode::UNAUTHORIZED, WebErr::SourceNotFound, "user not found")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	set_http_status_mode(HttpStatusMode::StatusCode);

	let app = Router::new()
		.route("/empty", get(ret_empty))
		.route("/user", get(get_user))
		.route("/user-null", get(user_null))
		.route("/user-error", get(user_err))
		.route("/user-auth-error", get(user_auth_err));

	let port = example_port()?;
	let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
	println!("Server running on http://127.0.0.1:{port}");
	axum::serve(TcpListener::bind(addr).await?, app.into_make_service()).await?;
	Ok(())
}

fn example_port() -> anyhow::Result<u16> {
	match env::var("AXUM_RESP_EXAMPLE_PORT") {
		Ok(port) => port
			.parse()
			.map_err(|err| anyhow::anyhow!("invalid AXUM_RESP_EXAMPLE_PORT `{port}`: {err}")),
		Err(env::VarError::NotPresent) => Ok(3100),
		Err(err) => Err(anyhow::anyhow!("cannot read AXUM_RESP_EXAMPLE_PORT: {err}")),
	}
}

#[cfg(test)]
mod tests {
	use axum::response::IntoResponse;
	use axum_resp_macro::resp_data;
	use infra_core::{err, result::SysErr};
	use serde::Serialize;
	use std::time::Duration;
	use tokio::time::sleep;

	#[derive(Debug, Serialize)]
	struct BalanceResp {
		user: String,
		balance: u64,
	}

	#[resp_data]
	async fn query_balance(user: String, should_fail: bool) -> AppResult<BalanceResp> {
		if should_fail {
			err!(SysErr::InvalidParams)
		} else {
			sleep(Duration::from_millis(10)).await;
			Ok(BalanceResp { user, balance: 42 })
		}
	}

	#[tokio::test]
	async fn test_resp_data() {
		let ok_resp = match query_balance("alice".into(), false).await {
			Ok(resp) => resp.into_response(),
			Err(err) => panic!("handler should succeed: {err}"),
		};
		println!("success http status: {}", ok_resp.status());

		match query_balance("bob".into(), true).await {
			Ok(resp) => {
				let resp = resp.into_response();
				println!("unexpected success http status: {}", resp.status());
			}
			Err(err) => {
				let resp = err.into_response();
				println!("error http status: {}", resp.status());
			}
		}
	}
}
