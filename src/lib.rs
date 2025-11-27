// SPDX-License-Identifier: MIT OR Apache-2.0
/*!
Thread-safe cell types for sending and sharing non-Send/non-Sync types across thread boundaries.

![logo](../../../art/logo.png)

This crate provides specialized cell types that allow you to work with types that don't normally
implement `Send` or `Sync` traits, enabling their use in concurrent contexts while maintaining
memory safety through either runtime checks or manual verification.

# Overview

The `send_cells` crate offers two categories of wrappers:

- **Safe wrappers** with runtime thread checking
- **Unsafe wrappers** for performance-critical scenarios with manual safety verification

This crate may be considered an alternative to the [fragile](https://crates.io/crates/fragile) crate,
but provides a more ergonomic API and additional unsafe variants for maximum performance.

# Quick Start

```rust
use send_cells::{SendCell, SyncCell};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

// Wrap a non-Send type to make it Send
let data = Rc::new(42);
let send_cell = SendCell::new(data);

// Access is checked at runtime - panics if accessed from wrong thread
assert_eq!(**send_cell.get(), 42);

// Wrap a non-Sync type to make it Sync
let shared_data = std::cell::RefCell::new("shared");
let sync_cell = Arc::new(SyncCell::new(shared_data));

// Share between threads with automatic synchronization
let sync_clone = Arc::clone(&sync_cell);
thread::spawn(move || {
    sync_clone.with(|data| {
        println!("Data: {}", data.borrow());
    });
}).join().unwrap();
```

# Safe Wrappers

Safe wrappers provide runtime-checked access to wrapped values:

## [`SendCell<T>`]

Allows sending non-Send types between threads with runtime thread checking:
- Remembers the thread it was created on
- Panics if accessed from a different thread
- Perfect for single-threaded async contexts

## [`SyncCell<T>`]

Allows sharing non-Sync types between threads with mutex-based synchronization:
- Uses internal mutex for thread-safe access
- Closure-based API prevents holding locks across await points
- Ideal for shared state in multi-threaded applications

## [`SendFuture<T>`]

Wraps non-Send futures to make them Send:
- Runtime checks ensure the future is only polled on the correct thread
- Enables use of non-Send futures with thread pool executors

# Unsafe Wrappers

Unsafe wrappers provide zero-cost abstractions when you can manually verify safety:

## [`UnsafeSendCell<T>`]

Allows sending non-Send types without runtime checks:
- No performance overhead
- Requires `unsafe` blocks for all access
- Suitable for platform-specific thread guarantees

## [`UnsafeSendFuture<T>`]

Wraps non-Send futures without runtime checks:
- Zero overhead compared to the underlying future
- Requires manual verification of thread safety

## [`UnsafeSyncCell<T>`]

Allows sharing non-Sync types without runtime checks:
- No synchronization overhead
- Requires `unsafe` blocks for all access
- Suitable when external synchronization is guaranteed

# When to Use Each Type

| Type | Use When | Performance | Safety |
|------|----------|------------|--------|
| `SendCell` | Moving non-Send types in async contexts | Good | Runtime checked |
| `SyncCell` | Sharing non-Sync types between threads | Good | Mutex protected |
| `SendFuture` | Using non-Send futures with Send requirements | Good | Runtime checked |
| `UnsafeSendCell` | Platform guarantees thread safety | Best | Manual verification |
| `UnsafeSyncCell` | External synchronization guarantees | Best | Manual verification |
| `UnsafeSendFuture` | Maximum performance for futures | Best | Manual verification |

# Platform Support

## Standard Platforms

Full support for all major platforms with standard library support.

## WebAssembly

This crate has full `wasm32-unknown-unknown` support with runtime thread checks
for web workers. Thread IDs are properly tracked even in WASM environments.

# Examples

## Async Runtime Integration

```
use send_cells::SendCell;
use std::rc::Rc;

async fn process_data() {
    // Rc is not Send, but we need to use it in an async context
    let data = Rc::new(vec![1, 2, 3]);
    let cell = SendCell::new(data);

    // Can be moved into async blocks that might run on different threads
    // Note: This would panic if actually polled on a different thread!
    let task = async move {
        // Will panic if actually polled on a different thread
        let data = cell.get();
        data.iter().sum::<i32>()
    };

    // In a real application with tokio:
    // let result = tokio::spawn(task).await.unwrap();
}
```

## Shared State with SyncCell

```rust
use send_cells::SyncCell;
use std::cell::RefCell;
use std::sync::Arc;
use std::thread;

let counter = RefCell::new(0);
let sync_counter = Arc::new(SyncCell::new(counter));

let mut handles = vec![];

for _ in 0..10 {
    let counter_clone = Arc::clone(&sync_counter);
    handles.push(thread::spawn(move || {
        counter_clone.with_mut(|counter| {
            *counter.borrow_mut() += 1;
        });
    }));
}

for handle in handles {
    handle.join().unwrap();
}

sync_counter.with(|counter| {
    assert_eq!(*counter.borrow(), 10);
});
```

## Platform-Specific Usage

```rust
use send_cells::UnsafeSendCell;
use std::rc::Rc;

// Platform API guarantees callbacks run on main thread
fn setup_main_thread_callback() {
    let data = Rc::new("main thread only");

    // SAFETY: Platform guarantees this callback runs on main thread
    let cell = unsafe { UnsafeSendCell::new_unchecked(data) };

    platform_specific_api(move || {
        // SAFETY: We're guaranteed to be on the main thread
        let data = unsafe { cell.get() };
        println!("Callback data: {}", data);
    });
}
# fn platform_specific_api<F: FnOnce() + Send + 'static>(_f: F) {}
```

# Safety Considerations

## Safe Wrappers

The safe wrappers (`SendCell`, `SyncCell`, `SendFuture`) provide memory safety through:
- Runtime thread checking with clear panic messages
- Automatic synchronization via mutexes
- Prevention of common concurrency bugs

## Unsafe Wrappers

The unsafe wrappers require manual verification of:
- Thread-local state dependencies
- Concurrent access patterns
- Drop safety on different threads
- External synchronization requirements

Always prefer safe wrappers unless you have specific performance requirements
and can rigorously verify thread safety.

# Performance

## Runtime Overhead

- **Safe wrappers**: Small overhead for thread ID checking or mutex operations
- **Unsafe wrappers**: Zero runtime overhead

## Memory Overhead

- **SendCell**: One `ThreadId` + wrapped value
- **SyncCell**: One `Mutex<()>` + wrapped value
- **UnsafeSendCell**: No overhead (transparent wrapper)

# Related Crates

- [fragile](https://crates.io/crates/fragile) - Similar functionality with different API design
- [once_cell](https://crates.io/crates/once_cell) - Lazy initialization primitives
- [parking_lot](https://crates.io/crates/parking_lot) - Alternative synchronization primitives
*/
pub mod send_cell;
pub mod sync_cell;
pub mod sys;
pub mod unsafe_send_cell;
pub mod unsafe_sync_cell;

pub use send_cell::{SendCell, SendFuture};
pub use sync_cell::SyncCell;
pub use unsafe_send_cell::{UnsafeSendCell, UnsafeSendFuture};
pub use unsafe_sync_cell::UnsafeSyncCell;
