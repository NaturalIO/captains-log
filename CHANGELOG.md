# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

### Removed

### Changed

### Fixed

## [0.5.1] 2025-07-15

### Changed

- Just refining API and usage document.

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
