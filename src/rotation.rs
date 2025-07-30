use file_rotate::SuffixInfo;
use file_rotate::compression::Compression;
use file_rotate::suffix::{
    AppendCount, AppendTimestamp, DateFrom, FileLimit, Representation, SuffixScheme,
};
use flate2::write::GzEncoder;
use parking_lot::Mutex;
use std::cell::UnsafeCell;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
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

    /// simular to system's log-rotate,
    /// For Age::Day, the latest archive use yesterday's timestamp;
    /// For Age::Hour, use last hour's timestamp.
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

    /// If None, archive in `file.<number>` form, and Upkeep::Age will be ignore.
    ///
    /// If Some, archive in `file.<datetime>` form.
    pub time_fmt: Option<&'static str>,

    /// How to cleanup the old file
    pub upkeep: Upkeep,

    /// Whether to move the log into an archive_dir. if not configured, it's the same dir as
    /// current log.
    pub archive_dir: Option<PathBuf>,

    /// When Some(count), indicate how many uncompressed archived logs. When 0, all the archive logs are compressed.
    /// When None, do not compress archive logs;
    pub compress_exclude: Option<usize>,
}

impl Rotation {
    /// max_files: When None, do not delete old files
    pub fn by_size(size_limit: u64, max_files: Option<usize>) -> Self {
        let upkeep =
            if let Some(_max_files) = max_files { Upkeep::Count(_max_files) } else { Upkeep::All };
        Self {
            by_age: None,
            by_size: Some(size_limit),
            time_fmt: None,
            upkeep,
            archive_dir: None,
            compress_exclude: None,
        }
    }

    pub fn by_age(
        age: Age, use_last_time: bool, time_fmt: &'static str, max_time: Option<chrono::TimeDelta>,
    ) -> Self {
        let upkeep =
            if let Some(_max_time) = max_time { Upkeep::Age(_max_time) } else { Upkeep::All };
        Self {
            by_age: Some(ByAge { age_type: age, use_last_time }),
            by_size: None,
            time_fmt: Some(time_fmt),
            upkeep,
            compress_exclude: None,
            archive_dir: None,
        }
    }

    /// Compress archived logs, with a number of recent files left uncompressed
    pub fn compress_exclude(mut self, un_compress_files: usize) -> Self {
        self.compress_exclude.replace(un_compress_files);
        self
    }

    /// Move the old logs into an `archive_dir`.
    pub fn archive_dir<P: Into<PathBuf>>(mut self, archive_dir: P) -> Self {
        self.archive_dir.replace(archive_dir.into());
        self
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
        let c = if let Some(compress) = &self.compress_exclude {
            Compression::OnRotate(*compress)
        } else {
            Compression::None
        };
        let backend;
        if let Some(time_fmt) = self.time_fmt {
            let file_limit = match self.upkeep {
                Upkeep::Age(d) => FileLimit::Age(d),
                Upkeep::Count(c) => FileLimit::MaxFiles(c),
                Upkeep::All => FileLimit::Unlimited,
            };
            let schema = AppendTimestamp { format: time_fmt, file_limit, date_from };
            backend = Backend::Time(UnsafeCell::new(_Backend::new(
                archive_dir.clone(),
                file_path,
                self.upkeep,
                c,
                schema,
            )));
        } else {
            let file_limit = match self.upkeep {
                Upkeep::Age(_) => 0,
                Upkeep::Count(c) => c,
                Upkeep::All => 0,
            };
            let schema = AppendCount::new(file_limit);
            backend = Backend::Num(UnsafeCell::new(_Backend::new(
                archive_dir.clone(),
                file_path,
                self.upkeep,
                c,
                schema,
            )));
        }
        return LogRotate {
            size_limit: size,
            age_limit: age,
            backend: Arc::new(backend),
            th: Mutex::new(None),
        };
    }
}

pub(crate) struct LogRotate {
    size_limit: Option<LimiterSize>,
    age_limit: Option<LimiterAge>,
    backend: Arc<Backend>,
    th: Mutex<Option<thread::JoinHandle<()>>>,
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
        self.wait();

        self.backend.rename_files();
        let backend = self.backend.clone();
        let th = thread::spawn(move || {
            let _ = backend.handle_old_files();
        });
        self.th.lock().replace(th);
        true
    }

    /// Wait for the last handle_old_files to finish.
    pub fn wait(&self) {
        if let Some(th) = self.th.lock().take() {
            let _ = th.join();
        }
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
    fn get_create_time(&self) -> SystemTime;

    fn get_size(&self) -> u64;
}

enum Backend {
    Num(UnsafeCell<_Backend<AppendCount>>),
    Time(UnsafeCell<_Backend<AppendTimestamp>>),
}

unsafe impl Send for Backend {}
unsafe impl Sync for Backend {}

impl Backend {
    fn rename_files(&self) {
        match self {
            Self::Num(_inner) => {
                let inner: &mut _Backend<AppendCount> = unsafe { transmute(_inner.get()) };
                inner.rename_files();
            }
            Self::Time(_inner) => {
                let inner: &mut _Backend<AppendTimestamp> = unsafe { transmute(_inner.get()) };
                inner.rename_files();
            }
        }
    }

    fn handle_old_files(&self) -> io::Result<()> {
        match self {
            Self::Num(_inner) => {
                let inner: &mut _Backend<AppendCount> = unsafe { transmute(_inner.get()) };
                inner.handle_old_files()
            }
            Self::Time(_inner) => {
                let inner: &mut _Backend<AppendTimestamp> = unsafe { transmute(_inner.get()) };
                inner.handle_old_files()
            }
        }
    }
}

/// Adaptation to file-rotate crate (Copyright (c) 2020 BourgondAries, MIT license)
struct _Backend<S: SuffixScheme> {
    archive_dir: PathBuf,
    base_path: PathBuf, // log patten in archive_dir
    log_path: PathBuf,  // current log
    compress: Compression,
    suffix_scheme: S,
    /// The bool is whether or not there's a .gz suffix to the filename
    suffixes: BTreeSet<SuffixInfo<S::Repr>>,
    upkeep: Upkeep,
}

fn compress(path: &Path) -> io::Result<()> {
    let dest_path = PathBuf::from(format!("{}.gz", path.display()));

    let mut src_file = File::open(path)?;
    let dest_file = OpenOptions::new().write(true).create(true).append(false).open(&dest_path)?;

    assert!(path.exists());
    assert!(dest_path.exists());
    let mut encoder = GzEncoder::new(dest_file, flate2::Compression::default());
    io::copy(&mut src_file, &mut encoder)?;

    fs::remove_file(path)?;

    Ok(())
}

impl<S: SuffixScheme> _Backend<S> {
    fn new(
        archive_dir: PathBuf, file: &Path, upkeep: Upkeep, compress: Compression, schema: S,
    ) -> Self {
        let base_path = archive_dir.as_path().join(Path::new(file.file_name().unwrap()));
        let mut s = Self {
            archive_dir,
            log_path: file.to_path_buf(),
            base_path,
            upkeep,
            compress,
            suffix_scheme: schema,
            suffixes: BTreeSet::new(),
        };
        s.ensure_dir();
        s.scan_suffixes();
        s
    }

    #[inline]
    fn ensure_dir(&self) {
        if !self.archive_dir.exists() {
            let _ = fs::create_dir_all(&self.archive_dir).expect("create dir");
        }
    }

    #[inline]
    fn scan_suffixes(&mut self) {
        self.suffixes = self.suffix_scheme.scan_suffixes(&self.base_path);
    }

    #[inline]
    fn rename_files(&mut self) {
        self.ensure_dir();
        let new_suffix_info = self._move_file_with_suffix(None).expect("move files");
        self.suffixes.insert(new_suffix_info);
    }

    #[inline]
    fn handle_old_files(&mut self) -> io::Result<()> {
        // Find the youngest suffix that is too old, and then remove all suffixes that are older or
        // equally old:
        // Start from oldest suffix, stop when we find a suffix that is not too old
        let mut result = Ok(());
        if let Upkeep::All = &self.upkeep {
        } else {
            let mut youngest_old = None;
            for (i, suffix) in self.suffixes.iter().enumerate().rev() {
                if self.suffix_scheme.too_old(&suffix.suffix, i) {
                    result = result.and(fs::remove_file(suffix.to_path(&self.base_path)));
                    youngest_old = Some((*suffix).clone());
                } else {
                    break;
                }
            }
            if let Some(youngest_old) = youngest_old {
                // Removes all the too old
                let _ = self.suffixes.split_off(&youngest_old);
            }
        }

        // Compression
        if let Compression::OnRotate(max_file_n) = self.compress {
            let n = (self.suffixes.len() as i32 - max_file_n as i32).max(0) as usize;
            // The oldest N files should be compressed
            let suffixes_to_compress = self
                .suffixes
                .iter()
                .rev()
                .take(n)
                .filter(|info| !info.compressed)
                .cloned()
                .collect::<Vec<_>>();
            for info in suffixes_to_compress {
                // Do the compression
                let path = info.suffix.to_path(&self.base_path);
                compress(&path)?;

                self.suffixes.replace(SuffixInfo { compressed: true, ..info });
            }
        }
        result
    }

    /// Recursive function that keeps moving files if there's any file name collision.
    /// If `suffix` is `None`, it moves from basepath to next suffix given by the SuffixScheme
    /// Assumption: Any collision in file name is due to an old log file.
    ///
    /// Returns the suffix of the new file (the last suffix after possible cascade of renames).
    fn _move_file_with_suffix(
        &mut self, old_suffix_info: Option<SuffixInfo<S::Repr>>,
    ) -> io::Result<SuffixInfo<S::Repr>> {
        // NOTE: this newest_suffix is there only because AppendTimestamp specifically needs
        // it. Otherwise it might not be necessary to provide this to `rotate_file`. We could also
        // have passed the internal BTreeMap itself, but it would require to make SuffixInfo `pub`.
        let newest_suffix = self.suffixes.iter().next().map(|info| &info.suffix);

        let new_suffix = self.suffix_scheme.rotate_file(
            &self.base_path,
            newest_suffix,
            &old_suffix_info.clone().map(|i| i.suffix),
        )?;

        // The destination file/path eventual .gz suffix must match the source path
        let new_suffix_info = SuffixInfo {
            suffix: new_suffix,
            compressed: old_suffix_info.as_ref().map(|x| x.compressed).unwrap_or(false),
        };
        let new_path = new_suffix_info.to_path(&self.base_path);

        // Whatever exists that would block a move to the new suffix
        let existing_suffix_info = self.suffixes.get(&new_suffix_info).cloned();

        // Move destination file out of the way if it exists
        let newly_created_suffix = if let Some(existing_suffix_info) = existing_suffix_info {
            // We might move files in a way that the destination path doesn't equal the path that
            // was replaced. Due to possible `.gz`, a "conflicting" file doesn't mean that paths
            // are equal.
            self.suffixes.replace(new_suffix_info);
            // Recurse to move conflicting file.
            self._move_file_with_suffix(Some(existing_suffix_info))?
        } else {
            new_suffix_info
        };

        let old_path = match old_suffix_info {
            Some(suffix) => suffix.to_path(&self.base_path),
            None => self.log_path.clone(), // When archive_dir and parent of log_path is different
        };
        // Do the move
        assert!(old_path.exists());
        assert!(!new_path.exists());
        fs::rename(old_path, new_path)?;

        Ok(newly_created_suffix)
    }
}
