use crate::{
    config::{LogConsole, LogFormat},
    log_impl::LogSinkTrait,
    time::Timer,
};
use log::{Level, Record};

pub struct LogSinkConsole {
    target_fd: libc::c_int,
    max_level: Level,
    formatter: LogFormat,
}

impl LogSinkConsole {
    pub fn new(config: &LogConsole) -> Self {
        Self {
            target_fd: config.target as i32,
            max_level: config.level,
            formatter: config.format.clone(),
        }
    }
}

impl LogSinkTrait for LogSinkConsole {
    fn reopen(&self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn log(&self, now: &Timer, r: &Record) {
        if r.level() <= self.max_level {
            let buf = self.formatter.process(now, r);
            unsafe {
                let _ = libc::write(self.target_fd, buf.as_ptr() as *const libc::c_void, buf.len());
            }
        }
    }
}
