# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

Use the helper scripts in `scripts/` for development. These handle platform differences and set correct flags.

```bash
# Run all checks (fmt, check, clippy, tests, docs) for both native and wasm32
./scripts/check_all

# Individual commands (run for both native and wasm32)
./scripts/check      # cargo check
./scripts/clippy     # cargo clippy
./scripts/tests      # cargo test
./scripts/docs       # cargo doc
./scripts/fmt        # cargo fmt --check

# Native-only or wasm32-only
./scripts/native/tests
./scripts/wasm32/tests

# Run a single test
cargo test test_name

# Use --relaxed to disable -D warnings (useful during development)
./scripts/tests --relaxed
```

## Requirements

- Rust 1.85.0+ (Rust 2024 edition)
- For WASM testing: `cargo +nightly`, `wasm-bindgen-test-runner`

## Architecture Overview

`send_cells` is a Rust library that provides cell types for safely sending and sharing non-Send/non-Sync types across thread boundaries. The library has two main categories:

### Safe Wrappers (Runtime-Checked)
- **`SendCell<T>`** (`src/send_cell.rs`): Wraps non-Send types with runtime thread checking. Panics if accessed from wrong thread.
- **`SyncCell<T>`** (`src/sync_cell.rs`): Wraps non-Sync types with mutex-based synchronization for safe concurrent access.
- **`SendFuture<T>`** (`src/send_cell.rs`): Wraps non-Send futures with runtime thread checking.

### Unsafe Wrappers (Zero-Cost)
- **`UnsafeSendCell<T>`** (`src/unsafe_send_cell.rs`): No runtime checks, requires unsafe blocks for access.
- **`UnsafeSyncCell<T>`** (`src/unsafe_sync_cell.rs`): No runtime checks for Sync types.
- **`UnsafeSendFuture<T>`** (`src/unsafe_send_cell.rs`): No runtime checks for futures.

### Platform Support
- **`src/sys.rs`**: Platform-specific thread ID implementation
- Special support for `wasm32-unknown-unknown` with web workers via `wasm_thread` dependency

## Key Design Patterns

1. **Thread Safety Model**: Safe wrappers store the creation thread ID and check it on every access. Unsafe wrappers bypass these checks for performance.

2. **API Design**: 
   - Safe wrappers provide `get()`, `with()`, `with_mut()` methods
   - Unsafe wrappers require explicit `unsafe` blocks
   - All types implement `Send` and/or `Sync` as appropriate

3. **Error Handling**: Safe wrappers panic with descriptive messages when accessed from wrong thread. Include thread IDs in panic messages for debugging.

4. **Documentation**: Every module has comprehensive rustdoc comments with examples. Documentation emphasizes safety requirements and use cases.

## Development Notes

- This is a low-level concurrency library - safety is paramount
- All unsafe code must have clear SAFETY comments explaining invariants
- Tests should verify both correct usage and panic cases for wrong-thread access
- Consider platform differences, especially WASM support with web workers