use crate::log_impl::LogSinkTrait;
use file_rotate::suffix::{AppendCount, AppendTimestamp, DateFrom, FileLimit};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Hash, Clone, Copy, PartialEq)]
pub enum Age {
    Day,
    Hour,
}

#[derive(Hash, Clone, Copy, PartialEq)]
pub struct ByAge {
    /// Rotate the file by day / hour.
    pub age_type: Age,

    /// In logrotate term, it's yesterday for Age::Day, last hour for Age::Hour.
    pub use_last_time: bool,
}

#[derive(Hash, Clone, Copy, PartialEq)]
pub enum Upkeep {
    /// Log file  older than the duration will be deleted.
    Age(chrono::TimeDelta),
    /// Only keeps the number of old logs.
    Count(usize),
    /// Does not delete any old logs.
    All,
}

/// Log rotation configuration.
///
/// `by_age` and `by_size` can be configured at the same time, means log will be rotate when any of the conditions met.
/// It's not valid when `by_age` and `by_size` both None.
#[derive(Hash)]
pub struct Rotation {
    pub by_age: Option<ByAge>,
    pub by_size: Option<u64>,

    /// If None, archive in "file.<number>" form, and Upkeep::Age will be ignore.
    ///
    /// If Some, archive in "file.<datetime>" form.
    pub time_fmt: Option<&'static str>,

    /// How to cleanup the old file
    pub upkeep: Upkeep,

    /// Whether to move the log into an archive_dir. if not configured, it's the same dir.
    pub archive_dir: Option<PathBuf>,
}

impl Rotation {
    pub fn by_size(size_limit: u64, max_files: usize, archive_dir: Option<PathBuf>) -> Self {
        Self {
            by_age: None,
            by_size: Some(size_limit),
            time_fmt: None,
            upkeep: Upkeep::Count(max_files),
            archive_dir,
        }
    }

    pub fn by_age(
        age: Age, use_last_time: bool, time_fmt: &'static str, max_time: chrono::TimeDelta,
        archive_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            by_age: Some(ByAge { age_type: age, use_last_time }),
            by_size: None,
            time_fmt: Some(time_fmt),
            upkeep: Upkeep::Age(max_time),
            archive_dir,
        }
    }

    pub(crate) fn build(&self, file_path: &Path) -> LogRotate {
        let archive_dir = if let Some(_dir) = &self.archive_dir {
            _dir.clone()
        } else {
            // TODO FIXME
            file_path.parent().unwrap().to_path_buf()
        };
        let mut size = None;
        let mut age = None;
        let mut date_from = DateFrom::Now;
        if let Some(by_age) = &self.by_age {
            if by_age.use_last_time {
                match by_age.age_type {
                    Age::Hour => {
                        date_from = DateFrom::DateHourAgo;
                    }
                    Age::Day => {
                        date_from = DateFrom::DateYesterday;
                    }
                }
            }
            age.replace(LimiterAge::new(by_age.age_type));
        }
        if let Some(_size) = &self.by_size {
            size.replace(LimiterSize::new(*_size));
        }
        let backend;
        if let Some(time_fmt) = self.time_fmt {
            let file_limit = match self.upkeep {
                Upkeep::Age(d) => FileLimit::Age(d),
                Upkeep::Count(c) => FileLimit::MaxFiles(c),
                Upkeep::All => FileLimit::Unlimited,
            };
            backend = Backend::Time(AppendTimestamp { format: time_fmt, file_limit, date_from });
        } else {
            let file_limit = match self.upkeep {
                Upkeep::Age(_) => 0,
                Upkeep::Count(c) => c,
                Upkeep::All => 0,
            };
            backend = Backend::Num(AppendCount::new(file_limit));
        }
        return LogRotate {
            archive_dir,
            size_limit: size,
            age_limit: age,
            backend,
            upkeep: self.upkeep,
        };
    }
}

enum Backend {
    Num(AppendCount),
    Time(AppendTimestamp),
}

pub(crate) struct LogRotate {
    archive_dir: PathBuf,
    size_limit: Option<LimiterSize>,
    age_limit: Option<LimiterAge>,
    backend: Backend,
    upkeep: Upkeep,
}

impl LogRotate {
    pub fn rotate<S: FileSinkTrait>(&self, sink: &S) -> bool {
        let mut need_rotate = false;
        if let Some(age) = self.age_limit.as_ref() {
            if age.check(sink) {
                need_rotate = true;
            }
        }
        if let Some(size) = self.size_limit.as_ref() {
            if size.check(sink) {
                need_rotate = true;
            }
        }
        if need_rotate == false {
            return false;
        }
        match &self.backend {
            Backend::Num(ac) => {}
            Backend::Time(at) => {}
        }
        true
    }
}

pub(crate) struct LimiterSize {
    limit: u64,
}

impl LimiterSize {
    pub fn new(size: u64) -> Self {
        Self { limit: size }
    }

    #[inline]
    pub fn check<S: FileSinkTrait>(&self, sink: &S) -> bool {
        return sink.get_size() > self.limit;
    }
}

pub(crate) struct LimiterAge {
    limit: Duration,
}

impl LimiterAge {
    pub fn new(limit: Age) -> Self {
        Self {
            limit: match limit {
                Age::Hour => Duration::from_secs(60 * 60),
                Age::Day => Duration::from_secs(24 * 60 * 60),
            },
        }
    }

    pub fn check<S: FileSinkTrait>(&self, sink: &S) -> bool {
        let now = SystemTime::now();
        let start_ts = sink.get_create_time();
        match now.duration_since(start_ts) {
            Ok(d) => return d > self.limit,
            Err(_) => return true, // system time rotate back
        }
    }
}

pub(crate) trait FileSinkTrait {
    fn get_file_path(&self) -> &Path;

    fn get_create_time(&self) -> SystemTime;

    fn get_size(&self) -> u64;
}
