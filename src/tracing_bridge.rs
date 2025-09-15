//! # Tracing support
//!
//! If you want to log tracing events (either in your code or 3rd-party crate), just enable the **`tracing` feature**.
//!
//! The message from tracing will use the same log format as defined in [crate::LogFormat].
//!
//! We suggest you should **opt out `tracing-log` from default feature-flag of `tracing_subscriber`**,
//! as it will conflict with captains-log. (It's not allowed to call `log::set_logger()` twice)
//!
//! ## Set global dispatcher (recommended)
//!
//! Just turn on the flag `tracing_global` in [crate::Builder], then it will setup [GlobalLogger] as the
//! default Subscriber.
//!
//! Error will be thrown by build() if other default subscribe has been set in tracing.
//!
//! ``` rust
//! use captains_log::*;
//! recipe::raw_file_logger("/tmp/mylog.log", Level::Debug)
//!                     .tracing_global()
//!                    .build().expect("setup log");
//! ```
//!
//! ## Stacking multiple layers (alternative)
//!
//! you can choose this method when you need 3rd-party layer
//! implementation.
//!
//! ```
//! use captains_log::*;
//! use tracing::{dispatcher, Dispatch};
//! use tracing_subscriber::{fmt, registry, prelude::*};
//! let logger = recipe::raw_file_logger("/tmp/tracing.log", Level::Trace)
//!                     .build().expect("setup logger");
//! // fmt::layer is optional
//! let reg = registry().with(fmt::layer().with_writer(std::io::stdout))
//!     .with(logger.tracing_layer().unwrap());
//! dispatcher::set_global_default(Dispatch::new(reg)).expect("init tracing");
//! ```
//!
//! ## Subscribe to tracing in the scope (rarely used).
//!
//! Assume you have a different tracing global dispatcher,
//! and want to output to captains_log only in the scope.
//! ```
//! use captains_log::*;
//! use tracing::{dispatcher, Dispatch};
//! use tracing_subscriber::{fmt, registry, prelude::*};
//!
//! let logger = recipe::raw_file_logger("/tmp/tracing.log", Level::Trace)
//!                     .build().expect("setup logger");
//! let reg = registry().with(fmt::layer().with_writer(std::io::stdout));
//! dispatcher::set_global_default(Dispatch::new(reg)).expect("init tracing");
//! tracing::trace!("trace with tracing {:?}", true);
//! let log_dispatch = logger.tracing_dispatch().unwrap();
//! dispatcher::with_default(&log_dispatch, || {
//!     tracing::info!("log from tracing in a scope");
//! });
//! ```

use crate::log::Log;
use crate::log_impl::GlobalLogger;
use log::Record;
use std::fmt::{self, Write};
use tracing::field::{Field, Visit};
use tracing::{span, Event, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// An tracing-subscriber layer implementation, for capturing event from tracing
pub struct CaptainsLogLayer<F = TracingText>
where
    F: TracingFormatter,
{
    /// This is a cycle pointer to the parent, to be filled after initialization.
    logger: &'static GlobalLogger,
    _phan: F,
}

unsafe impl<F: TracingFormatter> Send for CaptainsLogLayer<F> {}
unsafe impl<F: TracingFormatter> Sync for CaptainsLogLayer<F> {}

macro_rules! log_span {
    ($logger: expr, $id: expr, $meta: expr, $action: expr, $v: expr) => {{
        let msg = $v.as_ref();
        if msg.len() == 0 {
            $logger.log(
                &Record::builder()
                    .level(convert_tracing_level($meta.level()))
                    .target($meta.target())
                    .module_path($meta.module_path())
                    .file($meta.file())
                    .line($meta.line())
                    .args(format_args!("span({}) {}", $id.into_u64(), $action))
                    .build(),
            );
        } else {
            $logger.log(
                &Record::builder()
                    .level(convert_tracing_level($meta.level()))
                    .target($meta.target())
                    .module_path($meta.module_path())
                    .file($meta.file())
                    .line($meta.line())
                    .args(format_args!("span({}) {}: {}", $id.into_u64(), $action, msg))
                    .build(),
            );
        }
    }};
}

impl<F> CaptainsLogLayer<F>
where
    F: TracingFormatter,
{
    #[inline(always)]
    pub(crate) fn new(logger: &'static GlobalLogger) -> Self {
        Self { logger, _phan: Default::default() }
    }
}

impl<S, F> Layer<S> for CaptainsLogLayer<F>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    F: TracingFormatter + 'static,
{
    #[inline(always)]
    fn enabled(&self, meta: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        convert_tracing_level(meta.level()) <= log::STATIC_MAX_LEVEL
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(id).expect("Span not found");
        let meta = data.metadata();
        let mut extensions = data.extensions_mut();
        if extensions.get_mut::<F>().is_none() {
            let mut v = F::default();
            attrs.record(&mut v);
            log_span!(self.logger, id, meta, "new", v);
            extensions.insert(v);
        }
    }

    #[inline]
    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let data = ctx.span(id).expect("Span not found");
        let meta = data.metadata();
        let mut extensions = data.extensions_mut();
        if let Some(v) = extensions.get_mut::<F>() {
            values.record(v);
            log_span!(self.logger, id, meta, "record", v);
        } else {
            let mut v = F::default();
            values.record(&mut v);
            log_span!(self.logger, id, meta, "record", v);
            extensions.insert(v);
        }
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(&id).expect("Span not found, this is a bug");
        let meta = data.metadata();
        let extensions = data.extensions();
        if let Some(v) = extensions.get::<F>() {
            log_span!(self.logger, id, meta, "enter", v);
        }
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(&id).expect("Span not found, this is a bug");
        let meta = data.metadata();
        let extensions = data.extensions();
        if let Some(v) = extensions.get::<F>() {
            log_span!(self.logger, id, meta, "exit", v);
        }
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(&id).expect("Span not found, this is a bug");
        let meta = data.metadata();
        let extensions = data.extensions();
        if let Some(v) = extensions.get::<F>() {
            log_span!(self.logger, id, meta, "close", v);
        }
    }

    #[inline(always)]
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = F::default();
        event.record(&mut v);
        self.logger.log(
            &Record::builder()
                .level(convert_tracing_level(meta.level()))
                .args(format_args!("{}", v.as_ref()))
                .target(meta.target())
                .module_path(meta.module_path())
                .file(meta.file())
                .line(meta.line())
                .build(),
        );
    }
}

#[inline(always)]
pub fn convert_tracing_level(level: &tracing::Level) -> log::Level {
    match *level {
        tracing::Level::TRACE => log::Level::Trace,
        tracing::Level::DEBUG => log::Level::Debug,
        tracing::Level::INFO => log::Level::Info,
        tracing::Level::WARN => log::Level::Warn,
        tracing::Level::ERROR => log::Level::Error,
    }
}

pub trait TracingFormatter: Visit + Default + AsRef<str> + Send + Sync + 'static {}

pub struct TracingText(String);

impl Visit for TracingText {
    fn record_str(&mut self, field: &Field, value: &str) {
        if self.0.len() == 0 {
            if field.name() == "message" {
                write!(self.0, "{}", value).unwrap();
                return;
            } else {
                write!(self.0, "{}={}", field.name(), value).unwrap();
            }
        } else {
            write!(self.0, ", {}={}", field.name(), value).unwrap();
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.0.len() == 0 {
            if field.name() == "message" {
                write!(self.0, "{:?}", value).unwrap();
            } else {
                write!(self.0, "{}={:?}", field.name(), value).unwrap();
            }
        } else {
            write!(self.0, ", {}={:?}", field.name(), value).unwrap();
        }
    }
}

impl Default for TracingText {
    #[inline(always)]
    fn default() -> Self {
        Self(String::new())
    }
}

impl AsRef<str> for TracingText {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl TracingFormatter for TracingText {}
