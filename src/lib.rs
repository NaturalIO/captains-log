extern crate log;
extern crate log_helper;
extern crate signal_hook;

#[macro_use]
extern crate enum_dispatch;

mod config;
mod time;
mod formatter;
mod file_impl;
mod log_impl;

pub mod recipe;
pub mod macros;

mod log_filter;

pub use log::{Level as LogLevel, LevelFilter as LogLevelFilter};
pub use log_helper::logfn;

pub use self::{
    config::{Builder, LogFile},
    formatter::LogFormat,
    log_filter::*,
    log_impl::{setup_log, log_panic},
};

#[cfg(test)]
mod tests;
