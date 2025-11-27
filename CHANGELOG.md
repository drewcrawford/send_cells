# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-08-12

### Added

- **SyncCell**: A brand new cell type that lets you share non-Sync types between threads with automatic mutex-based synchronization. Perfect for when you need that RefCell accessible from multiple threads, but without the headaches. Features a closure-based API that keeps you from accidentally holding locks across await points.

- **SendFuture**: Wraps non-Send futures to make them Send-compatible. Now you can use those thread-local futures with thread pool executors, complete with runtime checks to make sure everything stays on the right thread.

- **Copying behaviors**: SendCell now supports Clone when appropriate, making it easier to work with duplicatable data.

- **into_future wrapper**: SendCell gained async support, so you can await it directly in your async code. One less step between you and your data.

### Changed

- **Edition upgrade**: Moved to Rust 2024 edition—keeping up with the times and taking advantage of the latest language improvements.

- **MSRV bump**: Now requires Rust 1.85.0 (up from 1.78.0). The new features needed some fresh compiler magic.

- **Documentation overhaul**: Completely rewrote the crate docs with clearer examples, better organization, and a handy comparison table to help you pick the right cell type for your use case. Also added more examples showing real-world usage patterns.

### Internal

- Standardized SPDX license headers across all source files
- Enhanced WebAssembly testing support with proper atomics handling
- Various CI improvements and refinements (we'll spare you the details of our caching adventures)

## [0.1.0] - 2025-01-30

Initial release!

### Added

- **SendCell**: Runtime-checked wrapper to send non-Send types between threads
- **UnsafeSendCell**: Zero-overhead wrapper for when you can guarantee thread safety yourself
- **UnsafeSyncCell**: Manual synchronization for the brave and careful
- Full WebAssembly support with proper thread tracking for web workers
- Comprehensive documentation and examples

[0.2.0]: https://github.com/drewcrawford/send_cells/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/drewcrawford/send_cells/releases/tag/v0.1.0
