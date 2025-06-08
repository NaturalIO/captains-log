#![recursion_limit = "128"]

//! #[logfn]
//! fn call_isan(num: &str) -> Result<Success, Error> {
//!     if num.len() >= 10 && num.len() <= 15 {
//!         Ok(Success)
//!     } else {
//!         Err(Error)
//!     }
//! }
//!
extern crate proc_macro;
extern crate syn;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, spanned::Spanned, token, AttributeArgs, Expr, ExprBlock, ExprClosure,
    ItemFn, Result, ReturnType,
};

struct FormattedAttributes {
    begin_expr: TokenStream,
    end_expr: TokenStream,
}

impl FormattedAttributes {
    fn get_streams(name: String) -> Self {
        let fmt_begin = format!("+++ {} begin +++", name);
        let fmt_end = format!("--- {} end ---", name);
        let begin_expr = quote! {log::info!(#fmt_begin); };
        let end_expr = quote! {log::info!(#fmt_end); };
        FormattedAttributes { begin_expr, end_expr }
    }
}

fn make_closure(original: &ItemFn) -> ExprClosure {
    let body = Box::new(Expr::Block(ExprBlock {
        attrs: Default::default(),
        label: Default::default(),
        block: *original.block.clone(),
    }));

    ExprClosure {
        attrs: Default::default(),
        asyncness: Default::default(),
        movability: Default::default(),
        capture: Some(token::Move { span: original.span() }),
        or1_token: Default::default(),
        inputs: Default::default(),
        or2_token: Default::default(),
        output: ReturnType::Default,
        body,
    }
}

fn replace_function_headers(original: ItemFn, new: &mut ItemFn) {
    let block = new.block.clone();
    *new = original;
    new.block = block;
}

fn generate_function(closure: &ExprClosure, expressions: FormattedAttributes) -> Result<ItemFn> {
    let FormattedAttributes { begin_expr, end_expr } = expressions;
    let code = quote! {
        fn temp() {
            #begin_expr;
            let _ = (#closure)();
            #end_expr;
        }
    };

    syn::parse2(code)
}

/// Logs the result of the function it's above.
/// # Examples
/// ``` rust
/// extern crate log_helper;
/// # use std::{net::*, io::{self, Write}};
/// #[logfn(err = "Error", fmt = "Failed Sending Packet: {:?}")]
/// fn send_hi(addr: SocketAddr) -> Result<(), io::Error> {
///     let mut stream = TcpStream::connect(addr)?;
///     stream.write(b"Hi!")?;
///     Ok( () )
/// }
///
///
/// ```
#[proc_macro_attribute]
pub fn logfn(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let _attr = parse_macro_input!(attr as AttributeArgs);
    let original_fn: ItemFn = parse_macro_input!(item as ItemFn);
    let parsed_attributes = FormattedAttributes::get_streams(original_fn.sig.ident.to_string());
    let closure = make_closure(&original_fn);
    let mut new_fn =
        generate_function(&closure, parsed_attributes).expect("Failed Generating Function");
    replace_function_headers(original_fn, &mut new_fn);
    new_fn.into_token_stream().into()
}
