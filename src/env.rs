use log::Level;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! impl_from_env {
    ($type: tt) => {
        impl<'a> Into<$type> for EnvVarDefault<'a, $type> {
            #[inline]
            fn into(self) -> $type {
                if let Ok(v) = std::env::var(&self.name) {
                    match $type::from_str(&v) {
                        Ok(r) => return r,
                        Err(_) => {
                            std::eprintln!(
                                "env {}={} is not valid, set to {:?}",
                                self.name,
                                v,
                                self.default
                            );
                        }
                    }
                }
                return self.default;
            }
        }
    };
}

pub struct EnvVarDefault<'a, T> {
    pub(crate) name: &'a str,
    pub(crate) default: T,
}

/// To config some logger setting with env.
///
/// Read value from environment, and set with default if not exists.
///
/// NOTE: the arguments to load from env_or() must support owned values.
///
/// Example:
///
/// ```rust
/// use captains_log::*;
/// let _level: log::Level = env_or("LOG_LEVEL", Level::Info).into();
/// let _file_path: String = env_or("LOG_FILE", "/tmp/test.log").into();
/// let _console: ConsoleTarget = env_or("LOG_CONSOLE", ConsoleTarget::Stdout).into();
/// ```
pub fn env_or<'a, T>(name: &'a str, default: T) -> EnvVarDefault<'a, T> {
    EnvVarDefault { name, default }
}

impl<'a> Into<String> for EnvVarDefault<'a, &'a str> {
    fn into(self) -> String {
        if let Ok(v) = std::env::var(&self.name) {
            return v;
        }
        return self.default.to_string();
    }
}

impl<'a, P: AsRef<Path>> Into<PathBuf> for EnvVarDefault<'a, P> {
    fn into(self) -> PathBuf {
        if let Some(v) = std::env::var_os(&self.name) {
            if v.len() > 0 {
                return PathBuf::from(v);
            }
        }
        return self.default.as_ref().to_path_buf();
    }
}

crate::impl_from_env!(Level);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe;
    use crate::*;

    #[test]
    fn test_env_config() {
        // test log level
        unsafe { std::env::set_var("LEVEL", "warn") };
        let level: Level = env_or("LEVEL", Level::Debug).into();
        assert_eq!(level, Level::Warn);
        unsafe { std::env::set_var("LEVEL", "WARN") };
        let level: Level = env_or("LEVEL", Level::Debug).into();
        assert_eq!(level, Level::Warn);

        assert_eq!(ConsoleTarget::from_str("Stdout").unwrap(), ConsoleTarget::Stdout);
        assert_eq!(ConsoleTarget::from_str("StdERR").unwrap(), ConsoleTarget::Stderr);
        assert_eq!(ConsoleTarget::from_str("1").unwrap(), ConsoleTarget::Stdout);
        assert_eq!(ConsoleTarget::from_str("2").unwrap(), ConsoleTarget::Stderr);
        assert_eq!(ConsoleTarget::from_str("0").unwrap_err(), ());

        // test console target
        unsafe { std::env::set_var("CONSOLE", "stderr") };
        let target: ConsoleTarget = env_or("CONSOLE", ConsoleTarget::Stdout).into();
        assert_eq!(target, ConsoleTarget::Stderr);
        unsafe { std::env::set_var("CONSOLE", "") };
        let target: ConsoleTarget = env_or("CONSOLE", ConsoleTarget::Stdout).into();
        assert_eq!(target, ConsoleTarget::Stdout);

        // test path
        unsafe { std::env::set_var("LOG_PATH", "/tmp/test.log") };
        let path: PathBuf = env_or("LOG_PATH", "/tmp/other.log").into();
        assert_eq!(path, Path::new("/tmp/test.log").to_path_buf());

        unsafe { std::env::set_var("LOG_PATH", "") };
        let path: PathBuf = env_or("LOG_PATH", "/tmp/other.log").into();
        assert_eq!(path, Path::new("/tmp/other.log").to_path_buf());

        let _builder = recipe::raw_file_logger(env_or("LOG_PATH", "/tmp/other.log"), Level::Info);
        let _builder =
            recipe::raw_file_logger(env_or("LOG_PATH", "/tmp/other.log".to_string()), Level::Info);
    }
}
