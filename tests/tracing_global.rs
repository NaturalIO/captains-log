use captains_log::*;
mod common;
use common::*;
use std::fs::*;

#[test]
fn test_tracing_global_dispatch() {
    lock_file!();
    let file_path = "/tmp/log_tracing.log";
    let _ = std::fs::remove_file(file_path);
    recipe::raw_file_logger(file_path, Level::Trace)
        .test()
        .tracing_global()
        .build()
        .expect("setup");
    log::debug!("debug with log crate");
    tracing::trace!("trace with tracing {:?}", true);

    #[tracing::instrument]
    fn tracing_instrument_test() {
        log::info!("a test");
    }
    tracing_instrument_test();

    let span_value = "a";
    let span = tracing::span!(tracing::Level::TRACE, "a span {}", span_value, key1 = 42, key2 = 20);
    {
        let _enter = span.enter();
    }

    tracing::event!(tracing::Level::INFO, message = "event with key", key1 = 33, key2 = "dfdf");

    tracing::info!("info with tracing {}", 2);

    println!("test setup twice");
    // because we detect our subscriber
    let _logger = recipe::raw_file_logger(file_path, Level::Trace)
        .test()
        .tracing_global()
        .build()
        .expect("setup");
}
