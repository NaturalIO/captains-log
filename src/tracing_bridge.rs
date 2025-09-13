use crate::log::Log;
use crate::log_impl::GlobalLogger;
use log::Record;
use std::fmt::{self, Write};
use tracing::field::{Field, Visit};
use tracing::{span, Event, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// An tracing-subscriber layer implementation, for capturing event from tracing
pub struct CaptainsLogLayer {
    /// This is a cycle pointer to the parent, to be filled after initialization.
    logger: &'static GlobalLogger,
}

unsafe impl Send for CaptainsLogLayer {}
unsafe impl Sync for CaptainsLogLayer {}

macro_rules! log_span {
    ($logger: expr, $id: expr, $meta: expr, $action: expr, $v: expr) => {{
        let msg = $v.as_str();
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

impl CaptainsLogLayer {
    #[inline(always)]
    pub(crate) fn new(logger: &'static GlobalLogger) -> Self {
        Self { logger }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for CaptainsLogLayer {
    #[inline(always)]
    fn enabled(&self, meta: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        convert_tracing_level(meta.level()) <= log::STATIC_MAX_LEVEL
    }

    #[inline]
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(id).expect("Span not found");
        let meta = data.metadata();
        let mut extensions = data.extensions_mut();
        if extensions.get_mut::<StringVisitor>().is_none() {
            let mut v = StringVisitor::new();
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
        if let Some(v) = extensions.get_mut::<StringVisitor>() {
            values.record(v);
            log_span!(self.logger, id, meta, "record", v);
        } else {
            let mut v = StringVisitor::new();
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
        if let Some(v) = extensions.get::<StringVisitor>() {
            log_span!(self.logger, id, meta, "enter", v);
        }
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(&id).expect("Span not found, this is a bug");
        let meta = data.metadata();
        let extensions = data.extensions();
        if let Some(v) = extensions.get::<StringVisitor>() {
            log_span!(self.logger, id, meta, "exit", v);
        }
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let data = ctx.span(&id).expect("Span not found, this is a bug");
        let meta = data.metadata();
        let extensions = data.extensions();
        if let Some(v) = extensions.get::<StringVisitor>() {
            log_span!(self.logger, id, meta, "close", v);
        }
    }

    #[inline(always)]
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = StringVisitor::new();
        event.record(&mut v);
        self.logger.log(
            &Record::builder()
                .level(convert_tracing_level(meta.level()))
                .args(format_args!("{}", v.as_str()))
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

struct StringVisitor(String);

impl Visit for StringVisitor {
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

impl StringVisitor {
    #[inline(always)]
    fn new() -> Self {
        Self(String::new())
    }

    #[inline(always)]
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
