use crate::{
    config::{SinkConfigBuild, SinkConfigTrait},
    log_impl::{LogSink, LogSinkTrait},
    time::Timer,
};
use crossfire::*;
use log::{Level, Record};
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Error, ErrorKind, Result, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use std::thread;
use std::time::{Duration, Instant};
pub use syslog::Facility;
use syslog::{Formatter3164, LogFormat as SyslogFormat, LoggerBackend as SyslogBackend, Severity};

const TIMEOUT_DEFAULT: Duration = Duration::from_secs(5);
const UNIX_SOCK_PATHS: [&str; 3] = ["/dev/log", "/var/run/syslog", "/var/run/log"];
// NOTE: local /dev/log is always available

const LOCAL_TCP: &'static str = "127.0.0.1:601";

#[allow(dead_code)]
const LOCAL_UDP: &'static str = "127.0.0.1:514";

#[derive(Hash)]
pub enum SyslogProto {
    RFC3164,
}
// NOTE the document of syslog crate does not tell much how to adapt Formatter5424 to log crate,
// so we only support 3164 for now.

#[derive(Hash, Clone, Debug)]
pub enum SyslogAddr {
    /// remote server addr
    TCP(String),
    /// local socket addr and remote server addr
    UDP(String, String),
    /// Unix with specified path
    Unix(PathBuf),
}

/// Config for syslog output, supports local and remote server.
///
/// The underlayer protocol is implemented by [syslog](https://docs.rs/syslog) crate,
/// currently Formatter3164 is adapted.
///
/// In order to achieve efficient socket I/O, the message is sent to channel,
/// and asynchronous flushed by backend writer.
///
/// **When your program shutting down, should call flush to ensure the log is written to the socket.**
///
/// ``` rust
/// log::logger().flush();
/// ```
/// On panic, our panic hook will call `flush()` explicitly.
///
/// On connection, will output "syslog connected" message to stdout.
///
/// On remote syslog server failure, will not panic, only "syslog: flush err" message will be print
/// to stderr, the backend thread will automatically reconnect to server.
/// In order to prevent hang up, the message will be dropped after a timeout.
///
/// # Example connecting local server
///
/// Source of [crate::recipe::syslog_local()]
///
/// ``` rust
/// use captains_log::*;
/// pub fn syslog_local(max_level: Level) -> Builder {
///     let syslog = Syslog::new(Facility::LOG_USER, max_level);
///     return Builder::default().add_sink(syslog);
/// }
/// ```
/// # Example connecting remote server
///
/// ``` rust
/// use captains_log::*;
/// let syslog = Syslog::new(Facility::LOG_USER, Level::Info).tcp("10.10.0.1:601");
/// let _ = Builder::default().add_sink(syslog).build();
/// ```
pub struct Syslog {
    /// Syslog facility
    pub facility: Facility,
    /// Auto filled current process
    pub process: Option<String>,
    /// Auto filled localhost,
    pub hostname: Option<String>,
    /// max level of message goes to syslog
    pub level: Level,
    /// When in doubt, use RFC3164
    pub proto: SyslogProto,
    /// When None, connect local default unix socket.
    pub server: Option<SyslogAddr>,
    /// Drop msg when syslog server fail after a timeout, also apply to tcp connect timeout.
    pub timeout: Duration,
}

impl Hash for Syslog {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_u32(self.facility as u32);
        self.process.hash(hasher);
        self.hostname.hash(hasher);
        self.level.hash(hasher);
        self.proto.hash(hasher);
        self.timeout.hash(hasher);
        self.server.hash(hasher);
    }
}

impl Default for Syslog {
    fn default() -> Self {
        Self {
            proto: SyslogProto::RFC3164,
            facility: Facility::LOG_USER,
            process: None,
            hostname: None,
            level: Level::Trace,
            timeout: TIMEOUT_DEFAULT,
            server: None,
        }
    }
}

impl Syslog {
    pub fn new(facility: Facility, level: Level) -> Self {
        let mut s = Self::default();
        s.proto = SyslogProto::RFC3164;
        s.facility = facility;
        s.level = level;
        s
    }

    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    /// Set hostname if you don't want the default
    pub fn hostname(mut self, name: String) -> Self {
        self.hostname = Some(name);
        self
    }

    /// Set process name if you don't want the default
    pub fn process_name(mut self, name: String) -> Self {
        self.process = Some(name);
        self
    }

    pub fn unix<P: Into<PathBuf>>(mut self, p: P) -> Self {
        self.server = Some(SyslogAddr::Unix(p.into()));
        self
    }

    pub fn tcp<S: AsRef<str>>(mut self, remote: S) -> Self {
        self.server = Some(SyslogAddr::TCP(remote.as_ref().to_string()));
        self
    }

    pub fn udp<S: AsRef<str>>(mut self, local: S, remote: S) -> Self {
        self.server =
            Some(SyslogAddr::UDP(local.as_ref().to_string(), remote.as_ref().to_string()));
        self
    }
}

impl SinkConfigBuild for Syslog {
    fn build(&self) -> LogSink {
        LogSink::Syslog(LogSinkSyslog::new(self))
    }
}

impl SinkConfigTrait for Syslog {
    fn get_level(&self) -> Level {
        self.level
    }

    fn get_file_path(&self) -> Option<Box<Path>> {
        None
    }

    fn write_hash(&self, hasher: &mut Box<dyn Hasher>) {
        self.hash(hasher);
        hasher.write(b"Syslog");
    }
}

enum Msg {
    Line(Vec<u8>),
    Flush(Arc<Once>),
}

pub(crate) struct LogSinkSyslog {
    tx: MTx<Msg>,
    format: Formatter3164,
    max_level: Level,
}

impl LogSinkSyslog {
    fn new(config: &Syslog) -> Self {
        let (tx, rx) = mpsc::bounded_blocking(100);

        macro_rules! fill_format {
            ($f: expr, $config: expr) => {{
                $f.facility = $config.facility;
                if $config.server.is_some() {
                    // remote
                    if let Some(hostname) = &$config.hostname {
                        $f.hostname = Some(hostname.clone());
                    }
                } else {
                    // local don't need hostname
                    $f.hostname = None;
                }
                if let Some(process) = &$config.process {
                    $f.process = process.clone();
                }
            }};
        }
        let mut timeout = config.timeout;
        if timeout == Duration::from_secs(0) {
            timeout = TIMEOUT_DEFAULT;
        }
        let mut backend = Backend { server: config.server.clone(), timeout, writer: None };
        let _ = backend.reinit();

        let mut f = Formatter3164::default();
        fill_format!(f, config);
        thread::spawn(move || backend.run(rx));
        Self { tx, max_level: config.level, format: f }
    }
}

impl LogSinkTrait for LogSinkSyslog {
    fn open(&self) -> std::io::Result<()> {
        Ok(())
    }

    fn reopen(&self) -> std::io::Result<()> {
        Ok(())
    }

    #[inline(always)]
    fn log(&self, _now: &Timer, r: &Record) {
        let l = r.level();
        if r.level() <= self.max_level {
            let mut buf = Vec::with_capacity(128);
            let _level = match l {
                Level::Trace => Severity::LOG_DEBUG, // syslog don't have trace level
                Level::Debug => Severity::LOG_DEBUG,
                Level::Info => Severity::LOG_INFO,
                Level::Warn => Severity::LOG_WARNING,
                Level::Error => Severity::LOG_ERR,
            };
            let msg = format!("{}", r.args());
            self.format.format(&mut buf, _level, msg).expect("format");
            let _ = self.tx.send(Msg::Line(buf));
        }
    }

    #[inline(always)]
    fn flush(&self) {
        let o = Arc::new(Once::new());
        if self.tx.send(Msg::Flush(o.clone())).is_ok() {
            o.wait();
        }
    }
}

struct Backend {
    server: Option<SyslogAddr>,
    writer: Option<SyslogBackend>,
    timeout: Duration,
}

impl Backend {
    #[inline]
    fn connect_unix(path: &Path) -> Result<SyslogBackend> {
        let sock = UnixDatagram::unbound()?;
        match sock.connect(Path::new(path)) {
            Ok(()) => {
                println!("syslog: connect to unix {:?}", path);
                return Ok(SyslogBackend::Unix(sock));
            }
            Err(e) => {
                if e.raw_os_error() == Some(libc::EPROTOTYPE) {
                    let sock = UnixStream::connect(path)?;
                    println!("syslog: connect to unix {:?}", path);
                    return Ok(SyslogBackend::UnixStream(BufWriter::new(sock)));
                }
                return Err(e);
            }
        }
    }

    #[inline]
    fn connect_tcp(s: &str, timeout: Duration) -> Result<SyslogBackend> {
        for addr in s.to_socket_addrs()? {
            let socket = TcpStream::connect_timeout(&addr, timeout)?;
            return Ok(SyslogBackend::Tcp(BufWriter::new(socket)));
        }
        Err(Error::new(ErrorKind::NotFound, "syslog: no server address").into())
    }

    #[inline]
    fn connect_udp(local: &str, remote: &str) -> Result<SyslogBackend> {
        let server_addr = remote.to_socket_addrs().and_then(|mut server_addr_opt| {
            server_addr_opt
                .next()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "syslog: no server address").into())
        })?;
        println!("syslog: connect to udp {:?}", remote);
        let socket = UdpSocket::bind(local)?;
        return Ok(SyslogBackend::Udp(socket, server_addr));
    }

    fn connect(server: &Option<SyslogAddr>, timeout: Duration) -> Result<SyslogBackend> {
        match server {
            Some(SyslogAddr::Unix(p)) => Self::connect_unix(p.as_path()),
            Some(SyslogAddr::UDP(local, remote)) => Self::connect_udp(&local, &remote),
            Some(SyslogAddr::TCP(remote)) => Self::connect_tcp(&remote, timeout),
            None => {
                for p in &UNIX_SOCK_PATHS {
                    if let Ok(backend) = Self::connect_unix(Path::new(p)) {
                        return Ok(backend);
                    }
                }
                return Self::connect_tcp(LOCAL_TCP, timeout);
                // Self::connect_udp("127.0.0.1:0", "127.0.0.1:514")
                // XXX: do not connect local udp unless specified by user,
                // since we have no means to detect udp failure
            }
        }
    }

    #[inline(always)]
    fn reinit(&mut self) -> Result<()> {
        match Self::connect(&self.server, self.timeout) {
            Err(e) => {
                eprintln!("syslog: connect {:?} err {:?}", e, self.server);
                return Err(e);
            }
            Ok(backend) => {
                self.writer = Some(backend);
                Ok(())
            }
        }
    }

    #[inline(always)]
    fn flush(&mut self) {
        if let Some(writer) = self.writer.as_mut() {
            if let Err(e) = writer.flush() {
                eprintln!("syslog: flush err {:?}", e);
                self.writer = None;
            }
        }
    }

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        if let Some(writer) = self.writer.as_mut() {
            match writer.write_all(msg) {
                Ok(_) => return,
                Err(e) => {
                    eprintln!("syslog: write err {:?}", e);
                    self.writer = None;
                }
            }
        }
        let start_ts = Instant::now();
        loop {
            thread::sleep(Duration::from_millis(500));
            if self.reinit().is_ok() {
                if let Some(writer) = self.writer.as_mut() {
                    match writer.write_all(msg) {
                        Ok(_) => return,
                        Err(e) => {
                            eprintln!("syslog: write err {:?}", e);
                            self.writer = None;
                        }
                    }
                }
            }
            if Instant::now().duration_since(start_ts) > self.timeout {
                // give up
                return;
            }
        }
    }

    fn run(&mut self, rx: Rx<Msg>) {
        loop {
            match rx.recv() {
                Ok(Msg::Line(_msg)) => {
                    self.write(&_msg);
                    let mut need_flush = true;
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            Msg::Line(_msg) => self.write(&_msg),
                            Msg::Flush(o) => {
                                self.flush();
                                o.call_once(|| {});
                                need_flush = false;
                            }
                        }
                    }
                    if need_flush {
                        self.flush();
                    }
                }
                Ok(Msg::Flush(o)) => {
                    self.flush();
                    o.call_once(|| {});
                }
                Err(_) => {
                    self.flush();
                    // exit
                    return;
                }
            }
        }
    }
}
