use captains_log::*;
mod common;
use common::*;
use std::fs::*;
use tracing::{dispatcher, Dispatch};
use tracing_subscriber::{fmt, prelude::*, registry};

#[test]
fn test_tracing_scope() {
    lock_file!();
    let file_path = "/tmp/log_tracing.log";
    let _ = std::fs::remove_file(file_path);
    let config = recipe::raw_file_logger(file_path, Level::Trace).test();
    assert_eq!(config.tracing_global, false);
    config.build().expect("setup logger");
    let reg = registry().with(fmt::layer().with_writer(std::io::stdout));
    // you should avoid calling init() in SubscriberInitExt trait,
    // because by tracing_subscriber has tracing-log as default feature,
    // it will failed when our logger already in log::set_logger().
    dispatcher::set_global_default(Dispatch::new(reg)).expect("init tracing");

    log::debug!("debug with log crate");
    tracing::trace!("trace with tracing {:?}", true);

    println!("test scopeed dispatcher");
    let log_dispatch = get_global_logger().unwrap().tracing_dispatch().unwrap();
    dispatcher::with_default(&log_dispatch, || {
        tracing::info!("log from tracing in a scope");
    });
}
