use std::thread;
use std::mem::transmute;
use std::sync::Arc;
use arc_swap::ArcSwap;
use backtrace::Backtrace;
use lazy_static::lazy_static;
use log::*;
use parking_lot::Mutex;
use signal_hook::iterator::Signals;
use crate::{
    config::Builder,
    file_impl::LoggerSinkFile,
    time::Timer,
};


#[enum_dispatch]
pub(crate) trait LoggerSinkTrait {

    fn reopen(&self) -> std::io::Result<()>;

    fn log(&self, now: &Timer, r: &Record);
}

#[enum_dispatch(LoggerSinkTrait)]
pub enum LoggerSink{
    File(LoggerSinkFile),
}

/// Global static structure to hold the logger
struct GlobalLogger {
    // Global static needs initialization when declaring,
    // default to be empty
    inner: Option<LoggerInner>,
}

enum LoggerInner {
    Once(Vec<LoggerSink>),
    // using ArcSwap has more cost
    Dyn(ArcSwap<Vec<LoggerSink>>),
}

fn panic_or_error() {
    #[cfg(debug_assertions)]
    {
        panic!("GlobalLogger cannot be initialized twice on dynamic==false");
    }
    #[cfg(not(debug_assertions))]
    {
        eprintln!("GlobalLogger cannot be initialized twice on dynamic==false");
    }
}


impl LoggerInner {

    #[allow(dead_code)]
    fn set(&self, sinks: Vec<LoggerSink>) {
        match &self {
            Self::Once(_)=>{
                panic_or_error();
            }
            Self::Dyn(d)=>{
                d.store(Arc::new(sinks));
            }
        }
    }
}

impl GlobalLogger {
    pub fn reopen(&mut self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner)=>{
                    for sink in inner.iter() {
                        sink.reopen()?;
                    }
                }
                LoggerInner::Dyn(inner)=>{
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.reopen()?;
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn init(&mut self, builder: &Builder) -> std::io::Result<bool> {
        if !builder.dynamic && self.inner.is_some() {
            panic_or_error();
            return Ok(false);
        }
        let mut sinks = Vec::new();
        for config in &builder.sinks {
            let logger_sink = config.build();
            logger_sink.reopen()?;
            sinks.push(logger_sink);
        }

        if let Some(inner) = self.inner.as_ref() {
            inner.set(sinks);
        } else {
            if builder.dynamic {
                self.inner.replace(LoggerInner::Dyn(ArcSwap::new(Arc::new(sinks))));
            } else {
                self.inner.replace(LoggerInner::Once(sinks));
            }
        }

        let _ = unsafe { set_logger(transmute::<&Self, &'static Self>(self)) };
        Ok(true)
    }
}

impl Log for GlobalLogger {
    fn enabled(&self, _m: &Metadata) -> bool {
        true
    }

    fn log(&self, r: &Record) {
        let now = Timer::new();
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner)=>{
                    for sink in inner.iter() {
                        sink.log(&now, r);
                    }
                }
                LoggerInner::Dyn(inner)=>{
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.log(&now, r);
                    }
                }
            }
        }
    }

    fn flush(&self) {}
}

lazy_static! {
    // Mutex only access on init and reopen, bypassed while logging,
    // because crate log only use const raw pointer to access GlobalLogger.
    static ref GLOBAL_LOGGER: Mutex<GlobalLogger> = Mutex::new(GlobalLogger { inner: None });
}

/// log handle for panic hook
#[doc(hidden)]
pub fn log_panic(info: &std::panic::PanicHookInfo) {
    let bt = Backtrace::new();
    let mut record = log::Record::builder();
    record.level(log::Level::Error);
    if let Some(loc) = info.location() {
        record.file(Some(loc.file())).line(Some(loc.line()));
    }
    log::logger().log(
        &record
            .args(format_args!("panic occur: {}\ntrace: {:?}", info, bt))
            .build(),
    );
    eprint!(
        "panic occur: {} at {:?}\ntrace: {:?}",
        info,
        info.location(),
        bt
    );
}

fn panic_and_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    std::process::exit(exitcode::IOERR);
}

fn panic_no_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    eprint!("not debug version, so don't exit process");
}

/// Initialize global logger from Builder
pub fn setup_log(builder: Builder) -> Result<(), ()> {
    {
        let mut global_logger = GLOBAL_LOGGER.lock();

        match global_logger.init(&builder) {
            Err(e) => {
                println!("Initialize logger failed: {:?}", e);
                return Err(());
            }
            Ok(false) => return Err(()),
            Ok(true) => {}
        }
        set_max_level(builder.get_max_level());
        if builder.continue_when_panic {
            std::panic::set_hook(Box::new(panic_no_exit_hook));
        } else {
            std::panic::set_hook(Box::new(panic_and_exit_hook));
        }
    }
    if builder.rotation_signals.len() > 0 {
        let signals = builder.rotation_signals.clone();
        thread::spawn(move || {
            let mut signals = Signals::new(&signals).unwrap();
            for __sig in signals.forever() {
                let mut global_logger = GLOBAL_LOGGER.lock();
                let _ = global_logger.reopen();
            }
        });
    }
    Ok(())
}
