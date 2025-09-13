use crate::{buf_file_impl::LogSinkBufFile, console_impl::LogSinkConsole, file_impl::LogSinkFile};
use crate::{config::Builder, time::Timer};
use arc_swap::ArcSwap;
use backtrace::Backtrace;
use signal_hook::iterator::Signals;
use std::cell::UnsafeCell;
use std::io::Error;
use std::mem::transmute;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::thread;

#[cfg(feature = "tracing")]
use crate::tracing_bridge::CaptainsLogLayer;
#[cfg(feature = "tracing")]
use tracing::{dispatcher, Dispatch};
#[cfg(feature = "tracing")]
use tracing_subscriber::prelude::*;

#[enum_dispatch]
pub(crate) trait LogSinkTrait {
    fn open(&self) -> std::io::Result<()>;

    fn reopen(&self) -> std::io::Result<()>;

    fn log(&self, now: &Timer, r: &log::Record);

    fn flush(&self);
}

#[enum_dispatch(LogSinkTrait)]
pub enum LogSink {
    File(LogSinkFile),
    BufFile(LogSinkBufFile),
    Console(LogSinkConsole),
    #[cfg(feature = "syslog")]
    Syslog(crate::syslog::LogSinkSyslog),
    #[cfg(feature = "ringfile")]
    RingFile(crate::ringfile::LogSinkRingFile),
}

struct GlobalLoggerStatic {
    logger: UnsafeCell<GlobalLogger>,
    lock: AtomicBool,
    inited: AtomicBool,
}

struct GlobalLoggerGuard<'a>(&'a GlobalLoggerStatic);

impl Drop for GlobalLoggerGuard<'_> {
    fn drop(&mut self) {
        self.0.unlock();
    }
}

impl GlobalLoggerStatic {
    const fn new() -> Self {
        Self {
            logger: UnsafeCell::new(GlobalLogger {
                config_checksum: AtomicU64::new(0),
                inner: None,
                signal_listener: AtomicBool::new(false),
                #[cfg(feature = "tracing")]
                tracing_inited: AtomicBool::new(false),
            }),
            lock: AtomicBool::new(false),
            inited: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    fn get_logger_mut(&self) -> &mut GlobalLogger {
        unsafe { transmute(self.logger.get()) }
    }

    /// Assume already setup, for internal use.
    #[inline(always)]
    fn get_logger(&'static self) -> &'static GlobalLogger {
        unsafe { transmute(self.logger.get()) }
    }

    /// This is the safe version for public use.
    #[inline(always)]
    fn try_get_logger(&'static self) -> Option<&'static GlobalLogger> {
        let logger = self.get_logger();
        if self.inited.load(Ordering::SeqCst) {
            return Some(logger);
        } else {
            None
        }
    }

    #[inline]
    fn lock<'a>(&'a self) -> GlobalLoggerGuard<'a> {
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Normally this does not contend, if your test does not run concurrently.
            std::thread::yield_now();
        }
        GlobalLoggerGuard(self)
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::SeqCst);
    }

    /// Return Ok(false) when reinit, Ok(true) when first init, Err for error
    fn try_setup(&'static self, builder: &Builder) -> Result<bool, Error> {
        let _guard = self.lock();
        let res = { self.get_logger().check_the_same(builder) };
        match res {
            Some(true) => {
                // checksum is the same
                if let Err(e) = self.get_logger().open() {
                    eprintln!("failed to open log sink: {:?}", e);
                    return Err(e);
                }
                return Ok(false);
            }
            Some(false) => {
                // checksum is not the same
                if !builder.dynamic {
                    let e = Error::other("log config differs but dynamic=false");
                    eprintln!("{:?}", e);
                    return Err(e);
                }
                let logger = self.get_logger();
                if let Err(e) = logger.reinit(builder) {
                    eprintln!("{:?}", e);
                    return Err(e);
                }
                // reset the log level
                log::set_max_level(builder.get_max_level());
                #[cfg(feature = "tracing")]
                {
                    if builder.tracing_global {
                        logger.init_tracing_global()?;
                    }
                }
                return Ok(false);
            }
            None => {
                // first init
                let res = { self.get_logger_mut().init(builder) };
                res?;
                #[cfg(feature = "tracing")]
                {
                    let logger = self.get_logger();
                    if builder.tracing_global {
                        logger.init_tracing_global()?;
                    }
                }
                self.inited.store(true, Ordering::SeqCst);
                return Ok(true);
            }
        }
    }
}

unsafe impl Send for GlobalLoggerStatic {}
unsafe impl Sync for GlobalLoggerStatic {}

/// Initialize global logger from Builder
///
/// **NOTE**: You can call this function multiple times when **builder.dynamic=true**,
/// but **cannot mixed used captains_log with other logger implement**, because log::set_logger()
/// cannot be called twice.
pub fn setup_log(builder: Builder) -> Result<&'static GlobalLogger, Error> {
    if GLOBAL_LOGGER.try_setup(&builder)? {
        let logger = GLOBAL_LOGGER.get_logger();
        // Set logger can only be called once
        if let Err(e) = log::set_logger(logger) {
            eprintln!("log::set_logger return error: {:?}", e);
            return Err(Error::other(format!("log::set_logger() failed: {:?}", e)));
        }
        log::set_max_level(builder.get_max_level());
        // panic hook can be set multiple times
        if builder.continue_when_panic {
            std::panic::set_hook(Box::new(panic_no_exit_hook));
        } else {
            std::panic::set_hook(Box::new(panic_and_exit_hook));
        }
        let signals = builder.rotation_signals.clone();
        if signals.len() > 0 {
            if false == logger.signal_listener.swap(true, Ordering::SeqCst) {
                thread::spawn(move || {
                    GLOBAL_LOGGER.get_logger().listener_for_signal(signals);
                });
            }
        }
        Ok(logger)
    } else {
        Ok(GLOBAL_LOGGER.get_logger())
    }
}

/// Return the GlobalLogger after initialized.
pub fn get_global_logger() -> Option<&'static GlobalLogger> {
    GLOBAL_LOGGER.try_get_logger()
}

/// Global static structure to hold the logger
pub struct GlobalLogger {
    /// checksum for config comparison
    config_checksum: AtomicU64,
    /// Global static needs initialization when declaring,
    /// default to be empty
    inner: Option<LoggerInner>,
    signal_listener: AtomicBool,
    #[cfg(feature = "tracing")]
    tracing_inited: AtomicBool,
}

enum LoggerInnerSink {
    Once(Vec<LogSink>),
    // using ArcSwap has more cost
    Dyn(ArcSwap<Vec<LogSink>>),
}

struct LoggerInner {
    sinks: LoggerInnerSink,
}

unsafe impl Send for LoggerInner {}
unsafe impl Sync for LoggerInner {}

impl LoggerInner {
    #[inline]
    fn new(dynamic: bool, sinks: Vec<LogSink>) -> Self {
        let sinks = if dynamic {
            LoggerInnerSink::Dyn(ArcSwap::new(Arc::new(sinks)))
        } else {
            LoggerInnerSink::Once(sinks)
        };
        Self { sinks }
    }

    #[inline]
    fn set(&self, sinks: Vec<LogSink>) -> std::io::Result<()> {
        match &self.sinks {
            LoggerInnerSink::Once(_) => {
                let e = Error::other("previous logger does not init with dynamic=true");
                return Err(e);
            }
            LoggerInnerSink::Dyn(d) => {
                d.store(Arc::new(sinks));
            }
        }
        Ok(())
    }
}

impl GlobalLogger {
    fn listener_for_signal(&self, signals: Vec<i32>) {
        println!("signal_listener started");
        let mut signals = Signals::new(&signals).unwrap();
        for __sig in signals.forever() {
            let _ = self.reopen();
        }
        println!("signal_listener exit");
    }

    /// On program/test Initialize
    fn open(&self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner.sinks {
                LoggerInnerSink::Once(inner) => {
                    for sink in inner.iter() {
                        sink.open()?;
                    }
                }
                LoggerInnerSink::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.open()?;
                    }
                }
            }
        }
        println!("log sinks opened");
        Ok(())
    }

    /// Reopen file. This is a signal handler, also can be called manually.
    pub fn reopen(&self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner.sinks {
                LoggerInnerSink::Once(inner) => {
                    for sink in inner.iter() {
                        sink.reopen()?;
                    }
                }
                LoggerInnerSink::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.reopen()?;
                    }
                }
            }
        }
        println!("log sinks re-opened");
        Ok(())
    }

    /// Return Some(true) to skip, Some(false) to reinit, None to init
    #[inline]
    fn check_the_same(&self, builder: &Builder) -> Option<bool> {
        if self.inner.is_some() {
            return Some(self.config_checksum.load(Ordering::Acquire) == builder.cal_checksum());
        }
        None
    }

    /// Re-configure the logger sink
    fn reinit(&self, builder: &Builder) -> std::io::Result<()> {
        let sinks = builder.build_sinks()?;
        if let Some(inner) = self.inner.as_ref() {
            inner.set(sinks)?;
            self.config_checksum.store(builder.cal_checksum(), Ordering::Release);
        } else {
            unreachable!();
        }
        Ok(())
    }

    fn init(&mut self, builder: &Builder) -> std::io::Result<()> {
        let sinks = builder.build_sinks()?;
        assert!(self.inner.is_none());
        self.inner.replace(LoggerInner::new(builder.dynamic, sinks));
        self.config_checksum.store(builder.cal_checksum(), Ordering::Release);
        Ok(())
    }

    #[cfg(feature = "tracing")]
    #[inline]
    fn init_tracing_global(&'static self) -> Result<(), Error> {
        let dist = self.tracing_dispatch()?;
        if let Err(_) = dispatcher::set_global_default(dist) {
            let e = Error::other("tracing global dispatcher already exists");
            eprintln!("{:?}", e);
            return Err(e);
        }
        self.tracing_inited.store(true, Ordering::SeqCst);
        Ok(())
    }

    #[cfg(feature = "tracing")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
    /// Initialize a layer for tracing. Use this when you stacking multiple tracing layers.
    ///
    /// For usage, checkout the doc in [crate::tracing_bridge]
    ///
    /// # NOTE:
    ///
    /// In order to prevent duplicate output, it will fail if out tracing global subscriber
    /// has been initialized.
    pub fn tracing_layer(&'static self) -> std::io::Result<CaptainsLogLayer> {
        if self.tracing_inited.load(Ordering::SeqCst) {
            let e = Error::other("global tracing dispatcher exists");
            eprintln!("{:?}", e);
            return Err(e);
        }
        return Ok(CaptainsLogLayer::new(self));
    }

    #[cfg(feature = "tracing")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
    /// Initialize a tracing Dispatch, you can set_global_default() or use in a scope.
    ///
    /// For usage, checkout the doc in [crate::tracing_bridge]
    ///
    /// # NOTE:
    ///
    /// In order to prevent duplicate output, it will fail if out tracing global subscriber
    /// has been initialized.
    pub fn tracing_dispatch(&'static self) -> std::io::Result<Dispatch> {
        if self.tracing_inited.load(Ordering::SeqCst) {
            let e = Error::other("global tracing dispatcher exists");
            eprintln!("{:?}", e);
            return Err(e);
        }
        return Ok(Dispatch::new(tracing_subscriber::registry().with(CaptainsLogLayer::new(self))));
    }
}

impl log::Log for GlobalLogger {
    #[inline(always)]
    fn enabled(&self, _m: &log::Metadata) -> bool {
        true
    }

    #[inline(always)]
    fn log(&self, r: &log::Record) {
        let now = Timer::new();
        if let Some(inner) = self.inner.as_ref() {
            match &inner.sinks {
                LoggerInnerSink::Once(inner) => {
                    for sink in inner.iter() {
                        sink.log(&now, r);
                    }
                }
                LoggerInnerSink::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.log(&now, r);
                    }
                }
            }
        }
    }

    /// Can be call manually on program shutdown (If you have a buffered log sink)
    ///
    /// # Example
    ///
    /// ``` rust
    /// log::logger().flush();
    /// ```
    fn flush(&self) {
        if let Some(inner) = self.inner.as_ref() {
            match &inner.sinks {
                LoggerInnerSink::Once(inner) => {
                    for sink in inner.iter() {
                        sink.flush();
                    }
                }
                LoggerInnerSink::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.flush();
                    }
                }
            }
        }
    }
}

static GLOBAL_LOGGER: GlobalLoggerStatic = GlobalLoggerStatic::new();

/// log handle for panic hook
#[doc(hidden)]
pub fn log_panic(info: &std::panic::PanicHookInfo) {
    let bt = Backtrace::new();
    let mut record = log::Record::builder();
    record.level(log::Level::Error);
    if let Some(loc) = info.location() {
        record.file(Some(loc.file())).line(Some(loc.line()));
    }
    log::logger().log(&record.args(format_args!("panic occur: {}\ntrace: {:?}", info, bt)).build());
    eprint!("panic occur: {} at {:?}\ntrace: {:?}", info, info.location(), bt);
}

#[inline(always)]
fn panic_and_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    log::logger().flush();
    let msg = format!("{}", info).to_string();
    std::panic::resume_unwind(Box::new(msg));
}

#[inline(always)]
fn panic_no_exit_hook(info: &std::panic::PanicHookInfo) {
    log_panic(info);
    eprint!("not debug version, so don't exit process");
    log::logger().flush();
}
