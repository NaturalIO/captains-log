use crate::{buf_file_impl::LogSinkBufFile, console_impl::LogSinkConsole, file_impl::LogSinkFile};
use crate::{config::Builder, time::Timer};
use arc_swap::ArcSwap;
use backtrace::Backtrace;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use signal_hook::iterator::Signals;
use std::mem::transmute;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

#[enum_dispatch]
pub(crate) trait LogSinkTrait {
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
}

/// Global static structure to hold the logger
struct GlobalLogger {
    /// checksum for config comparison
    config_checksum: u64,
    /// Global static needs initialization when declaring,
    /// default to be empty
    inner: Option<LoggerInner>,
    signal_listener: AtomicBool,
}

enum LoggerInner {
    Once(Vec<LogSink>),
    // using ArcSwap has more cost
    Dyn(ArcSwap<Vec<LogSink>>),
}

#[inline(always)]
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
    fn set(&self, sinks: Vec<LogSink>) {
        match &self {
            Self::Once(_) => {
                panic_or_error();
            }
            Self::Dyn(d) => {
                d.store(Arc::new(sinks));
            }
        }
    }
}

impl GlobalLogger {
    pub fn reopen(&mut self) -> std::io::Result<()> {
        if let Some(inner) = self.inner.as_ref() {
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.reopen()?;
                    }
                }
                LoggerInner::Dyn(inner) => {
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

    #[allow(dead_code)]
    fn init(&mut self, builder: &Builder) -> std::io::Result<bool> {
        let new_checksum = builder.cal_checksum();
        if self.inner.is_some() {
            if self.config_checksum == new_checksum {
                // Config is the same, no need to reinit
                self.reopen()?;
                return Ok(true);
            }
            if !builder.dynamic {
                panic_or_error();
                return Ok(false);
            }
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
        self.config_checksum = new_checksum;

        let _ = unsafe { log::set_logger(transmute::<&Self, &'static Self>(self)) };

        // panic hook can be set multiple times
        if builder.continue_when_panic {
            std::panic::set_hook(Box::new(panic_no_exit_hook));
        } else {
            std::panic::set_hook(Box::new(panic_and_exit_hook));
        }
        Ok(true)
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
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.log(&now, r);
                    }
                }
                LoggerInner::Dyn(inner) => {
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
            match &inner {
                LoggerInner::Once(inner) => {
                    for sink in inner.iter() {
                        sink.flush();
                    }
                }
                LoggerInner::Dyn(inner) => {
                    let sinks = inner.load();
                    for sink in sinks.iter() {
                        sink.flush();
                    }
                }
            }
        }
    }
}

lazy_static! {
    // Mutex only access on init and reopen, bypassed while logging,
    // because crate log only use const raw pointer to access GlobalLogger.
    static ref GLOBAL_LOGGER: Mutex<GlobalLogger> = Mutex::new(GlobalLogger {
        config_checksum: 0,
        inner: None ,
        signal_listener: AtomicBool::new(false),
    });
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

fn signal_listener(signals: Vec<i32>) {
    let started;
    {
        let global_logger = GLOBAL_LOGGER.lock();
        started = global_logger.signal_listener.swap(true, Ordering::SeqCst);
    }
    if started {
        // NOTE: Once logger started to listen signal, does not support dynamic reconfigure.
        eprintln!("signal listener already started");
        return;
    }
    thread::spawn(move || {
        let mut signals = Signals::new(&signals).unwrap();
        for __sig in signals.forever() {
            {
                let mut global_logger = GLOBAL_LOGGER.lock();
                let _ = global_logger.reopen();
            }
        }
    });
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
        log::set_max_level(builder.get_max_level());
    }
    let signals = builder.rotation_signals.clone();
    if signals.len() > 0 {
        signal_listener(signals);
    }
    Ok(())
}
