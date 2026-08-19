use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ReturnType, Type, TypePath, parse_macro_input};

#[proc_macro_attribute]
pub fn resp_data(_args: TokenStream, input: TokenStream) -> TokenStream {
	let fnc = parse_macro_input!(input as ItemFn);

	let inner_ty = match parse_return_type(&fnc) {
		Ok(inner) => inner,
		Err(err) => return err.to_compile_error().into(),
	};

	TokenStream::from(expand_resp_data(fnc, inner_ty))
}

fn expand_resp_data(mut fnc: ItemFn, inner_ty: Type) -> proc_macro2::TokenStream {
	fnc.sig.output = syn::parse_quote! {
		-> ::infra_web::resp::AxumResult<impl ::axum::response::IntoResponse>
	};

	let block = fnc.block;
	fnc.block = syn::parse_quote!({
		let res: #inner_ty = (async #block).await?;
		::infra_web::success!(res)
	});

	quote! {
		#fnc
	}
}

fn parse_return_type(fnc: &ItemFn) -> Result<Type, syn::Error> {
	let output = match &fnc.sig.output {
		ReturnType::Type(_, ty) => ty,
		_ => {
			return Err(syn::Error::new_spanned(
				&fnc.sig.output,
				"resp_data requires a return type like AppResult<T> or AxumResult<T>",
			));
		}
	};

	let output: &Type = output;
	match output {
		Type::Path(tp) => extract_resp_result(output, tp),
		_ => Err(syn::Error::new_spanned(
			output,
			"Return type must be AppResult<T> or AxumResult<T>",
		)),
	}
}

fn extract_resp_result(output: &Type, tp: &TypePath) -> Result<Type, syn::Error> {
	let Some(segment) = tp.path.segments.last() else {
		return Err(syn::Error::new_spanned(
			output,
			"Return type must be AppResult<T> or AxumResult<T>",
		));
	};

	if segment.ident != "AppResult" && segment.ident != "AxumResult" {
		return Err(syn::Error::new_spanned(
			output,
			"Return type must be AppResult<T> or AxumResult<T>",
		));
	}
	match &segment.arguments {
		syn::PathArguments::AngleBracketed(ab) => {
			if ab.args.len() != 1 {
				return Err(syn::Error::new_spanned(
					ab,
					"AppResult<T> or AxumResult<T> must have exactly one generic parameter",
				));
			}

			match ab.args.first() {
				Some(syn::GenericArgument::Type(t)) => Ok(t.clone()),
				Some(inner_ty) => Err(syn::Error::new_spanned(inner_ty, "Invalid generic type")),
				None => Err(syn::Error::new_spanned(
					ab,
					"AppResult<T> or AxumResult<T> must have exactly one generic parameter",
				)),
			}
		}
		_ => Err(syn::Error::new_spanned(
			segment,
			"AppResult<T> or AxumResult<T> must have generic parameter",
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::{expand_resp_data, parse_return_type};
	use quote::{ToTokens, quote};
	use syn::{ItemFn, Type, parse_quote};

	fn parse_inner_type(return_ty: Type) -> Type {
		let fnc: ItemFn = parse_quote! {
			async fn handler() -> #return_ty {
				todo!()
			}
		};

		match parse_return_type(&fnc) {
			Ok(inner) => inner,
			Err(err) => panic!("return type should parse: {err}"),
		}
	}

	#[test]
	fn parse_bare_app_result_inner_type() {
		let inner = parse_inner_type(parse_quote!(AppResult<Option<User>>));

		assert_eq!(inner.to_token_stream().to_string(), "Option < User >");
	}

	#[test]
	fn parse_qualified_app_result_inner_type() {
		let inner = parse_inner_type(parse_quote!(infra_core::result::AppResult<()>));

		assert_eq!(inner.to_token_stream().to_string(), "()");
	}

	#[test]
	fn parse_bare_axum_result_inner_type() {
		let inner = parse_inner_type(parse_quote!(AxumResult<Option<User>>));

		assert_eq!(inner.to_token_stream().to_string(), "Option < User >");
	}

	#[test]
	fn parse_qualified_axum_result_inner_type() {
		let inner = parse_inner_type(parse_quote!(infra_web::resp::AxumResult<()>));

		assert_eq!(inner.to_token_stream().to_string(), "()");
	}

	#[test]
	fn rejects_non_app_result_return_type() {
		let fnc: ItemFn = parse_quote! {
			async fn handler() -> Result<(), AppError> {
				Ok(())
			}
		};

		let err = match parse_return_type(&fnc) {
			Ok(_) => panic!("Result should be rejected"),
			Err(err) => err,
		};
		assert!(err.to_string().contains("AppResult<T> or AxumResult<T>"));
	}

	#[test]
	fn expands_to_infra_web_axum_result_and_success_macro() {
		let fnc: ItemFn = parse_quote! {
			async fn handler() -> AppResult<User> {
				load_user().await
			}
		};
		let inner = match parse_return_type(&fnc) {
			Ok(inner) => inner,
			Err(err) => panic!("return type should parse: {err}"),
		};
		let output = expand_resp_data(fnc, inner).to_string();

		assert!(output.contains(":: infra_web :: resp :: AxumResult"));
		assert!(output.contains(":: infra_web :: success !"));
		assert!(output.contains("let res : User"));
	}

	#[test]
	fn generated_body_awaits_original_async_block_then_question_marks() {
		let fnc: ItemFn = parse_quote! {
			async fn handler() -> AppResult<()> {
				do_work().await?;
				Ok(())
			}
		};
		let inner = match parse_return_type(&fnc) {
			Ok(inner) => inner,
			Err(err) => panic!("return type should parse: {err}"),
		};
		let output = expand_resp_data(fnc, inner).to_string();

		assert!(output.contains("(async"));
		assert!(output.contains(". await ?"));
		assert!(output.contains(quote!(let res: ()).to_string().as_str()));
	}
}
