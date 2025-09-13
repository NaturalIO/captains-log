use captains_log::*;
mod common;
use common::*;
use std::fs::*;
use tracing::{dispatcher, Dispatch};
use tracing_subscriber::{fmt, prelude::*, registry};

#[test]
fn test_tracing_layer() {
    lock_file!();
    let file_path = "/tmp/log_tracing.log";
    let _ = std::fs::remove_file(file_path);

    log::debug!("debug with log crate");
    tracing::trace!("trace with tracing {:?}", true);
    let logger =
        recipe::raw_file_logger(file_path, Level::Trace).test().build().expect("setup logger");
    let reg = registry()
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(logger.tracing_layer().unwrap());
    // you should avoid calling init() in SubscriberInitExt trait,
    // because by tracing_subscriber has tracing-log as default feature,
    // it will failed when our logger already in log::set_logger().
    dispatcher::set_global_default(Dispatch::new(reg)).expect("init tracing");
}
