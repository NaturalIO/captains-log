//! This is sub-crate of [captains-log](https://docs.rs/captains-log/latest/captains_log/)
//!
//! Provides a attribute macro [macro@logfn] to wrap function for convenience.

#![recursion_limit = "128"]
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Expr,
    ItemFn, LitStr, Ident, Signature, FnArg, Pat, PatType,
    Block, Stmt, ExprCall,ExprAsync, Generics, Path,
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

// The following code reused the `async_trait` probes from tokio-tracing
//
// https://github.com/tokio-rs/tracing/blob/6a61897a/tracing-attributes/src/expand.rs
// Copyright (c) 2019 Tokio Contributors, MIT license
fn process_async_trait<'a>(block: &'a Block, is_async: bool) -> Option<&'a ExprAsync> {

    fn path_to_string(path: &Path) -> String {
        use std::fmt::Write;
        let mut res = String::with_capacity(path.segments.len() * 5);
        for i in 0..path.segments.len() {
            write!(res, "{}", path.segments[i].ident).expect("write to string ok");
            if i < path.segments.len() -1 {
                res.push_str("::");
            }
        }
        res
    }

    if is_async {
        return None;
    }
    // last expression of the block (it determines the return value
    // of the block, so that if we are working on a function whose
    // `trait` or `impl` declaration is annotated by async_trait,
    //  this is quite likely the point where the future is pinned)
    let last_expr = block.stmts.iter().rev().find_map(|stmt| {
        if let Stmt::Expr(expr, ..) = stmt {
            Some(expr)
        } else {
            None
        }
    })?;
    // is the last expression a function call?
    let (outside_func, outside_args) = match last_expr {
        Expr::Call(ExprCall { func, args, .. }) => (func, args),
        _ => return None,
    };
    // is it a call to `Box::pin()`?
    let path = match outside_func.as_ref() {
        Expr::Path(path) => &path.path,
        _ => return None,
    };
    if !path_to_string(path).ends_with("Box::pin") {
        return None;
    }
    // Does the call take an argument? If it doesn't,
    // it's not going to compile anyway, but that's no reason
    // to (try to) perform an out-of-bounds access
    if outside_args.is_empty() {
        return None;
    }
    // Is the argument to Box::pin an async block that
    // captures its arguments?
    if let Expr::Async(async_expr) = &outside_args[0] {
         // check that the move 'keyword' is present
        async_expr.capture?;
        return Some(async_expr);
    }
    unimplemented!("async-trait < 0.1.44 is not supported");
}


fn generate_function(args: Args, block: &Block,
    async_context: bool, async_keyword: bool,
    sig: &Signature) -> proc_macro2::TokenStream {
    let fn_name = sig.ident.to_string();
    let level = args.level.unwrap_or("info".to_string());
    let level = Ident::new(&level, Span::call_site());
    let arg_list = gen_arg_list(sig);
    let fmt_begin = format!("<<< {} ({}) enter <<<", fn_name, arg_list);
    let fmt_end = format!(">>> {} return {{__ret_value:?}} in {{__dur:?}} >>>", fn_name);
    let begin_expr = quote! {
        log::#level!(#fmt_begin);
        let __start_ts = std::time::Instant::now();
    };
    let end_expr = quote! {
        let __dur = std::time::Instant::now() - __start_ts;
        log::#level!(#fmt_end);
    };

    if async_context {
        let block = quote::quote_spanned!(block.span()=>
            #begin_expr
            let __ret_value = async { #block }.await;
            #end_expr
            __ret_value
        );
        if async_keyword { // normal async fn
            return block.into();
        } else { // async_trait
            return quote::quote_spanned!(block.span()=>
                async move {
                    #block
                }
            ).into();
        }
    } else {
        return quote::quote_spanned!(block.span()=>
            #begin_expr
            let __ret_value = (move ||#block)();
            #end_expr
            __ret_value
        ).into();
    }
}

fn output_stream(input: &ItemFn, func_body: proc_macro2::TokenStream) -> proc_macro::TokenStream {
    let sig = &input.sig;
    let attrs = &input.attrs;
    let vis = &input.vis;
    let Signature {
        output,
        inputs,
        unsafety,
        constness,
        abi,
        ident,
        asyncness,
        generics:
            Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;
    quote::quote_spanned!(input.span()=>
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#inputs) #output
        #where_clause
        {
            #func_body
        }
    ).into()
}


/// Provide an proc_macro `#[logfn]` which log the infomation:
///
/// - function call begin, argument list
///
/// - function return value, duration time.
///
/// Supports async fn and async-trait >=0.1.44.
///
/// # Examples
///
/// ``` rust
///
/// use captains_log::{recipe, logfn};
/// use log::*;
///
/// let builder = recipe::raw_file_logger("/tmp", "log_test", log::Level::Debug).test();
/// builder.build().expect("setup_log");
///
/// // default log level to be info
/// #[logfn]
/// fn foo() {
///     info!("foo");
///     bar(1, "bar arg");
/// }
///
/// // you can change log level to warn
/// #[logfn(warn)]
/// fn bar(a: i32, s: &str) {
///     info!("bar");
/// }
/// foo();
/// ```
///
/// /tmp/log_test.log will have this content:
///
/// ``` text
///[2025-07-13 18:22:39.151481][INFO][test_logfn.rs:19] <<< foo () enter <<<
///[2025-07-13 18:22:39.151774][INFO][test_logfn.rs:21] foo
///[2025-07-13 18:22:39.151840][WARN][test_logfn.rs:25] <<< bar (a = 1, s = "bar arg") enter <<<
///[2025-07-13 18:22:39.151886][INFO][test_logfn.rs:27] bar
///[2025-07-13 18:22:39.151925][WARN][test_logfn.rs:25] >>> bar return () in 39.183µs >>>
///[2025-07-13 18:22:39.151969][INFO][test_logfn.rs:19] >>> foo return () in 197.381µs >>>
/// ```
///
/// # Best practice with test suit
///
/// You can have `#[logfn]` used with retest.
///
/// * When you have large test suit, you want to know which logs belong to which test case.
///
/// * Sometimes your test crashes, you want to find the responsible test case.
///
/// * The time spend in each test
///
/// ``` rust
/// use rstest::*;
/// use log::*;
/// use captains_log::*;
///
/// // A show case that setup() fixture will be called twice, before each test.
/// // In order make logs available.
/// #[fixture]
/// fn setup() {
///     let builder = recipe::raw_file_logger("/tmp", "log_rstest", log::Level::Debug).test();
///     builder.build().expect("setup_log");
/// }
///
/// #[logfn]
/// #[rstest(file_size, case(0), case(1))]
/// fn test_rstest_foo(setup: (), file_size: usize) {
///     info!("do something111");
/// }
///
/// #[logfn]
/// #[rstest]
/// fn test_rstest_bar(setup: ()) {
///     info!("do something222");
/// }
///
/// #[tokio::test]
/// #[logfn]
/// #[rstest]
/// async fn test_rstest_async(setup: ()) {
///     info!("something333")
/// }
///
/// ```
///
/// **Notice:** the order when combine tokio::test with rstest,
/// `#[rstest]` attribute must be at the bottom to make setup fixture effective.
///
/// After running the test with:
/// `cargo test -- --test-threads=1`
///
/// /tmp/log_rstest.log will have this content:
///
/// ``` text
///[2025-07-13 18:22:39.159642][INFO][test_rstest.rs:33] <<< test_rstest_async (setup = ()) enter <<<
///[2025-07-13 18:22:39.160255][INFO][test_rstest.rs:37] something333
///[2025-07-13 18:22:39.160567][INFO][test_rstest.rs:33] >>> test_rstest_async return () in 564.047µs >>>
///[2025-07-13 18:22:39.161299][INFO][test_rstest.rs:26] <<< test_rstest_bar (setup = ()) enter <<<
///[2025-07-13 18:22:39.161643][INFO][test_rstest.rs:29] do something222
///[2025-07-13 18:22:39.161703][INFO][test_rstest.rs:26] >>> test_rstest_bar return () in 62.681µs >>>
///[2025-07-13 18:22:39.162169][INFO][test_rstest.rs:20] <<< test_rstest_foo (setup = (), file_size = 0) enter <<<
///[2025-07-13 18:22:39.162525][INFO][test_rstest.rs:23] do something111
///[2025-07-13 18:22:39.162600][INFO][test_rstest.rs:20] >>> test_rstest_foo return () in 78.457µs >>>
///[2025-07-13 18:22:39.163050][INFO][test_rstest.rs:20] <<< test_rstest_foo (setup = (), file_size = 1) enter <<<
///[2025-07-13 18:22:39.163320][INFO][test_rstest.rs:23] do something111
///[2025-07-13 18:22:39.163377][INFO][test_rstest.rs:20] >>> test_rstest_foo return () in 58.747µs >>>
/// ```
#[proc_macro_attribute]
pub fn logfn(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let original_fn: ItemFn = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(attr as Args);
//    let closure = make_closure(&original_fn);
    let is_async = original_fn.sig.asyncness.is_some();

    let body = {
        if let Some(async_expr) = process_async_trait(&original_fn.block, is_async) {
            let inst_block = generate_function(args, &async_expr.block, true, false, &original_fn.sig);
            let async_attrs = &async_expr.attrs;
            quote::quote_spanned! {async_expr.span()=>
                Box::pin(#(#async_attrs) * #inst_block)}
        } else {
            generate_function(
                args, &original_fn.block, is_async, is_async, &original_fn.sig)
        }
    };
    output_stream(&original_fn, body)
}
