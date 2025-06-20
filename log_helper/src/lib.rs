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

/// Provide an proc_macro `#[logfn]` which log the function call begin and return,
/// with argument list and return value.
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
/// [2025-06-21 01:00:17.116049][INFO][test_logfn.rs:13] <<< foo () enter <<<
/// [2025-06-21 01:00:17.116206][INFO][test_logfn.rs:15] foo
/// [2025-06-21 01:00:17.116223][WARN][test_logfn.rs:19] <<< bar (a = 1, s = "bar arg") enter <<<
/// [2025-06-21 01:00:17.116236][INFO][test_logfn.rs:21] bar
/// [2025-06-21 01:00:17.116246][WARN][test_logfn.rs:19] >>> bar return () >>>
/// [2025-06-21 01:00:17.116256][INFO][test_logfn.rs:13] >>> foo return () >>>
/// ```
///
/// # Best practice with test suit
///
/// Nice to have `#[logfn]` used with retest.
///
/// * When you have large test suit, you want to know which logs belong to which test case.
///
/// * Sometimes your test crashes, you want to find the resposible test case.
///
/// ```
/// use rstest::*;
/// use log::*;
/// use captains_log::*;
///
/// // A show case that setup() fixture will be called twice, before each test.
/// // In order make logs available.
/// #[logfn]
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
/// ```
///
/// After running the test with:
/// `cargo test -- --test-threads=1`
///
/// /tmp/log_rstest.log will have this content:
///
/// ``` text
/// [2025-06-21 00:39:37.091326][INFO][test_rstest.rs:11] >>> setup return () >>>
/// [2025-06-21 00:39:37.091462][INFO][test_rstest.rs:27] <<< test_rstest_bar (setup = ()) enter <<<
/// [2025-06-21 00:39:37.091493][INFO][test_rstest.rs:30] do something222
/// [2025-06-21 00:39:37.091515][INFO][test_rstest.rs:27] >>> test_rstest_bar return () >>>
/// [2025-06-21 00:39:37.091719][INFO][test_rstest.rs:11] <<< setup () enter <<<
/// [2025-06-21 00:39:37.091826][INFO][test_rstest.rs:11] >>> setup return () >>>
/// [2025-06-21 00:39:37.091844][INFO][test_rstest.rs:21] <<< test_rstest_foo (setup = (), file_size = 0) enter <<<
/// [2025-06-21 00:39:37.091857][INFO][test_rstest.rs:24] do something111
/// [2025-06-21 00:39:37.091868][INFO][test_rstest.rs:21] >>> test_rstest_foo return () >>>
/// [2025-06-21 00:39:37.092063][INFO][test_rstest.rs:11] <<< setup () enter <<<
/// [2025-06-21 00:39:37.092136][INFO][test_rstest.rs:11] >>> setup return () >>>
/// [2025-06-21 00:39:37.092151][INFO][test_rstest.rs:21] <<< test_rstest_foo (setup = (), file_size = 1) enter <<<
/// [2025-06-21 00:39:37.092163][INFO][test_rstest.rs:24] do something111
/// [2025-06-21 00:39:37.092173][INFO][test_rstest.rs:21] >>> test_rstest_foo return () >>>
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
