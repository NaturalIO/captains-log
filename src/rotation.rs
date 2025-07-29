use std::time::{Duration, Instant};

#[derive(Hash, Clone, Copy)]
pub enum Age {
    Day,
    Hour,
}

#[derive(Hash)]
pub struct ByAge {
    /// Rotate the file by day / hour.
    t: Age,

    /// In logrotate term, it's yesterday for Age::Day, last hour for Age::Hour.
    last_time: bool,
}

#[derive(Hash)]
pub enum Upkeep {
    /// Log file  older than the duration will be deleted.
    Age(Duration),
    /// Only keeps the number of old logs.
    Count(usize),
}

#[derive(Hash)]
pub struct Rotation {
    pub by_age: Option<Age>,
    pub by_size: Option<usize>,

    /// How to cleanup the old file.
    pub upkeep: Option<Upkeep>,

    /// If None, archive in "file.<number>" form. If Some, archive in "file.<datetime>" form.
    pub time_fmt: Option<&'static str>,
}

pub(crate) struct RotationLimiterSize {
    cur: usize,
    limit: usize,
}

impl RotationLimiterSize {
    pub fn new(size: usize) -> Self {
        Self { cur: 0, limit: size }
    }

    #[inline]
    pub fn add(&mut self, size: usize) -> bool {
        self.cur += size;
        return self.cur > self.limit;
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.cur = 0;
    }
}

pub(crate) struct RotationLimiterAge {
    start: Instant,
    limit: Duration,
}

impl RotationLimiterAge {
    pub fn new(limit: Age) -> Self {
        Self {
            start: Instant::now(),
            limit: match limit {
                Age::Hour => Duration::from_secs(60 * 60),
                Age::Day => Duration::from_secs(24 * 60 * 60),
            },
        }
    }

    pub fn check(&mut self) -> bool {
        let now = Instant::now();
        if now < self.start {
            // system time rotate back
            return true;
        }
        let d = now - self.start;
        return d > self.limit;
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}

pub(crate) struct CleanupFile {}
