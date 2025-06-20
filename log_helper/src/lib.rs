#![recursion_limit = "128"]
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, spanned::Spanned, token, Expr, ExprBlock, ExprClosure,
    ItemFn, Result, ReturnType, LitStr, Ident, Signature, FnArg, Pat, PatType,
};
use syn::parse::{Parse, ParseStream};

#[derive(Default, Debug)]
struct Args {
    level: Option<String>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result::<Self> {
        let mut args = Self::default();
        while !input.is_empty() {
            if input.peek(LitStr) {
                // [logfn("warn")]
                let level = input.parse::<LitStr>()?;
                args.level = Some(level.value());
            } else if input.peek(Ident) {
                // [logfn(warn)]
                let ident = input.parse::<Ident>()?;
                if args.level.is_none() {
                    args.level = Some(ident.to_string());
                }
            } else {
                return Err(input.error("unexpected token"));
            }
        }
        if let Some(v) = args.level.as_ref() {
            match v.as_str() {
                "trace" | "debug" | "info" | "warn" | "error"=>{},
                _=> {
                    return Err(input.error(format!("unexpected level value: {:?}", v)));
                }
            }
        }
        Ok(args)
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
        lifetimes: Default::default(),
        constness: Default::default(),
        movability: Default::default(),
        asyncness: Default::default(),
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

fn gen_arg_list(sig: &Signature) -> String {
    let mut arg_list = String::new();
    for (i, input) in sig.inputs.iter().enumerate() {
        if i > 0 {
            arg_list.push_str(", ");
        }
        match input {
            FnArg::Typed(PatType { pat, .. }) => {
                if let Pat::Ident(pat_ident) = &**pat {
                    let ident = &pat_ident.ident;
                    arg_list.push_str(&format!("{ident} = {{{ident}:?}}"));
                }
            }
            FnArg::Receiver(_) => {
                arg_list.push_str("self");
            }
        }
    }
    arg_list
}

fn generate_function(closure: &ExprClosure, args: Args, fn_name: String, sig: &Signature) -> Result<ItemFn> {
    let level = args.level.unwrap_or("info".to_string());
    let level = Ident::new(&level, Span::call_site());
    let arg_list = gen_arg_list(sig);
    let fmt_begin = format!("<<< {} ({}) enter <<<", fn_name, arg_list);
    let fmt_end = format!(">>> {} return {{__ret_value:?}} >>>", fn_name);
    let begin_expr = quote! {log::#level!(#fmt_begin); };
    let end_expr = quote! {log::#level!(#fmt_end); };
    let code = quote! {
        fn temp() {
            #begin_expr;
            let __ret_value = (#closure)();
            #end_expr;
        }
    };

    syn::parse2(code)
}

/// Logs the result of the function it's above.
/// # Examples
///
/// ``` rust
/// #[macro_use]
/// extern crate captains_log;
/// # use std::{net::*, io::{self, Write}};
///
/// #[logfn]
/// fn call_isan(num: &str) -> Result<Success, Error> {
///     if num.len() >= 10 && num.len() <= 15 {
///         Ok(Success)
///     } else {
///         Err(Error)
///     }
/// }
///
/// #[logfn(err = "Error", fmt = "Failed Sending Packet: {:?}")]
/// fn send_hi(addr: SocketAddr) -> Result<(), io::Error> {
///     let mut stream = TcpStream::connect(addr)?;
///     stream.write(b"Hi!")?;
///     Ok( () )
/// }
///
/// ```
#[proc_macro_attribute]
pub fn logfn(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let original_fn: ItemFn = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(attr as Args);
    let fn_name = original_fn.sig.ident.to_string();
    let closure = make_closure(&original_fn);
    let mut new_fn =
        generate_function(&closure, args, fn_name, &original_fn.sig).expect("Failed Generating Function");
    replace_function_headers(original_fn, &mut new_fn);
    new_fn.into_token_stream().into()
}
