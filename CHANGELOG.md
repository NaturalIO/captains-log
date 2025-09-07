# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

### Removed

### Changed

### Fixed

## [0.9.4] 2025-09-08

### Fixed

- RingFile: Try fix the unsoundness on open()

## [0.9.3] 2025-09-07

### Fixed

- RingFile: Fix block when using stdout

## [0.9.2] 2025-09-07

### Fixed

- RingFile: clear content on open()

## [0.9.1] 2025-09-06

### Fixed

- RingFile: call exit after dump()

## [0.9.0] 2025-09-06

### Added

- Support thread_id in LogFormat

### Changed

- RingFile: A refactor, to store the buffer in thread local, remove lock contention to avoid
affecting the thread scheduling. Supports output to stdout.
 NOTE that the buf_size parameter has changed to size of per-thread.

### Fixed

- LogRawFile and LogBufFile now handle short writes.

## [0.8.5] 2025-08-28

### Changed

- RingFile: Trigger dump from panic hook to support debugging assertion.

## [0.8.4] 2025-08-07

### Fixed

- Fix mirror warning about setup_log()

## [0.8.1-0.8.3] 2025-08-06

### Fixed

- Fix the doc of ringfile feature.

## [0.8.0] 2025-08-03

### Added

- Add LogSinkTrait::open(), to be distinguish with reopen().

- Add RingFile sink as a debugging tool for deadlock

- Re-export signal_consts for convenience

### Changed

- Remove Builder::add_XXX functions, replace them with add_sink()

## [0.7.0] 2025-07-31

### Added

- Implement syslog feature, supports local and remote server, with timeout and auto reconnect.

### Changed

- Removed all `continue_when_panic` determination in recipe function

### Fixed

- LogBufFile: flush should wait for backend

## [0.6.2] 2025-07-31

### Fixed

- Fix config hash for LogBufFile sink

- Make flush time configurable for LogBufFile

## [0.6.1] 2025-07-30

### Added

- Add Rotation::by_age_and_size()

## [0.6.0] 2025-07-30

### Added

- Add Buffered file sink with optional rotation (depends on file-rotate crate)

## [0.5.2] 2025-07-15

### Added

- Add macro log_eprintln!()

### Changed

- refining API and usage document.

## [0.5.0] 2025-07-15

### Added

- Support configure from environment with env_or(), and recipe::env_logger().

### Changed

- Change recipe raw_file_logger() & raw_file_custom_logger(), user need to specified the full path.

- Change the definition of LogRawFile to support path/str/String/env_or().

## [0.4.7] 2025-07-13

### Added

- Update macro attribute logfn to support async fn, adding time duration in logs

## [0.4.2-0.4.6] 2025-06-28

- Refine document, regarding rstest usage.

## [0.4.1] 2025-06-21

### Added

- Update macro attribute logfn to log function argument and return values.

## [0.4.0] 2025-06-20

### Added

- Add Builder::test() for test cases.

### Changed

- API refactor:  Rename file()->raw_file(), LogFile -> LogRawFile.

- Do not reload when log config is not changed in test cases, will not panic even without dynamic=true.

## [0.3.0] 2025-06-11

### Added

- Add a simple LogParser with regex

## [0.2.0] 2025-06-10

### Added

- Add LogFilterKV which inherit LogFilter

- Add console output support

- Various recipes
