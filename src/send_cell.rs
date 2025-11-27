// SPDX-License-Identifier: MIT OR Apache-2.0
/*!
A runtime-checked cell for safely sending non-Send types across thread boundaries.

This module provides [`SendCell<T>`] and [`SendFuture<T>`], which allow you to wrap
non-Send types and move them between threads while ensuring thread safety through runtime checks.
Unlike [`crate::unsafe_send_cell`], this module provides safe APIs that panic if accessed
from the wrong thread.

# Use Cases

- Wrapping non-Send types (like `Rc<T>`, `RefCell<T>`) to use in async contexts that require Send
- Moving platform-specific resources between threads when you know it's safe
- Prototyping concurrent code without fighting the borrow checker
- Working with callback-based APIs where thread guarantees are implicit

# Thread Safety Model

[`SendCell<T>`] remembers the thread it was created on and performs runtime checks on all access:
- All methods except the `*_unchecked` variants will panic if called from a different thread
- The cell can be moved between threads, but can only be accessed from its origin thread
- Drop is also checked, ensuring the wrapped value is only dropped on the correct thread

# Example

```rust
use send_cells::SendCell;
use std::rc::Rc;

// Rc<T> is not Send, but we can wrap it in SendCell
let data = Rc::new(42);
let cell = SendCell::new(data);

// The cell itself implements Send
fn requires_send<T: Send>(_: T) {}
requires_send(cell);

// Access the data (only works on the original thread)
// let value = *cell.get(); // Would work here
// println!("Value: {}", value);
```

# Futures

[`SendFuture<T>`] provides the same thread safety guarantees for futures:

```rust
use send_cells::SendCell;
use std::rc::Rc;
use std::future::Future;

// A future that contains non-Send data
async fn non_send_future() -> i32 {
    let _local = Rc::new(42); // Non-Send
    42
}

// Wrap it to make it Send with runtime checks
let future = non_send_future();
let cell = SendCell::new(future);
let send_future = cell.into_future();

// Now it can be used in Send contexts
fn requires_send_future<F: Future + Send>(_: F) {}
requires_send_future(send_future);
```
*/

use crate::sys::thread::ThreadId;
use crate::unsafe_send_cell::UnsafeSendCell;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

/// A runtime-checked cell that allows sending non-Send types between threads.
///
/// `SendCell<T>` wraps a value of type `T` (which may not implement `Send`) and provides
/// a `Send` implementation with runtime thread checking. The cell remembers the thread
/// it was created on and panics if accessed from any other thread.
///
/// Unlike [`crate::UnsafeSendCell`], this provides memory safety by performing runtime
/// checks on all operations. This makes it safe to use but comes with the cost of
/// runtime panics if used incorrectly.
///
/// # Examples
///
/// Basic usage with a non-Send type:
///
/// ```rust
/// use send_cells::SendCell;
/// use std::rc::Rc;
///
/// // Rc<i32> is not Send, but SendCell<Rc<i32>> is
/// let data = Rc::new(42);
/// let cell = SendCell::new(data);
///
/// // Access the wrapped value
/// assert_eq!(**cell.get(), 42);
///
/// // The cell can be moved between threads (but not accessed)
/// fn assert_send<T: Send>(_: T) {}
/// assert_send(cell);
/// ```
///
/// Cloning/copying wrapped values:
///
/// ```rust
/// use send_cells::SendCell;
///
/// let cell = SendCell::new(42i32);
/// let copied_cell = cell.copying(); // Safe for Copy types
///
/// assert_eq!(*cell.get(), *copied_cell.get());
/// ```
///
/// # Panics
///
/// All methods (except `*_unchecked` variants) will panic if called from a different
/// thread than the one where the `SendCell` was created.
pub struct SendCell<T> {
    inner: Option<UnsafeSendCell<T>>,
    thread_id: ThreadId,
}

impl<T> SendCell<T> {
    /// Creates a new `SendCell` wrapping the given value.
    ///
    /// The cell will "remember" the current thread ID. All subsequent access
    /// to the wrapped value will be checked against this thread ID, and will
    /// panic if accessed from a different thread.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::rc::Rc;
    ///
    /// let data = Rc::new("Hello, world!");
    /// let cell = SendCell::new(data);
    ///
    /// // Can access on the same thread
    /// println!("{}", cell.get());
    /// ```
    #[inline]
    pub fn new(t: T) -> SendCell<T> {
        SendCell {
            //safe because drop is verified
            inner: Some(unsafe { UnsafeSendCell::new_unchecked(t) }),
            thread_id: crate::sys::thread::current().id(),
        }
    }

    /// Unsafely accesses the underlying value without thread checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The value is safe to access from the current thread
    /// - No concurrent access is occurring from other threads
    /// - The value's invariants are maintained
    ///
    /// This method bypasses the runtime thread check and may lead to undefined
    /// behavior if the wrapped type is not actually thread-safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    ///
    /// let cell = SendCell::new(42);
    ///
    /// // SAFETY: We're on the same thread, so this is safe
    /// let value = unsafe { cell.get_unchecked() };
    /// assert_eq!(*value, 42);
    /// ```
    #[inline]
    pub unsafe fn get_unchecked(&self) -> &T {
        unsafe { self.inner.as_ref().expect("gone").get() }
    }
    /// Accesses the underlying value with runtime thread checking.
    ///
    /// This is the safe way to access the wrapped value. The method will verify
    /// that the current thread matches the thread where the cell was created.
    ///
    /// # Panics
    ///
    /// Panics if called from a different thread than the one where this `SendCell`
    /// was created.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("key", "value");
    /// let cell = SendCell::new(map);
    ///
    /// // Safe access on the same thread
    /// let value = cell.get().get("key");
    /// assert_eq!(value, Some(&"value"));
    /// ```
    #[inline]
    pub fn get(&self) -> &T {
        assert_eq!(
            self.thread_id,
            crate::sys::thread::current().id(),
            "Access SendCell<{}> from incorrect thread",
            std::any::type_name::<T>()
        );
        //safe with assertion
        unsafe { self.get_unchecked() }
    }

    /// Unsafely accesses the underlying value mutably without thread checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The value is safe to access mutably from the current thread
    /// - No concurrent access is occurring from other threads
    /// - The value's invariants are maintained after mutation
    ///
    /// This method bypasses the runtime thread check and may lead to undefined
    /// behavior if the wrapped type is not actually thread-safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    ///
    /// let mut cell = SendCell::new(42);
    ///
    /// // SAFETY: We're on the same thread, so this is safe
    /// unsafe {
    ///     *cell.get_unchecked_mut() = 100;
    /// }
    /// assert_eq!(*cell.get(), 100);
    /// ```
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        unsafe { &mut *self.inner.as_mut().expect("gone").get_mut() }
    }

    /// Accesses the underlying value mutably with runtime thread checking.
    ///
    /// This is the safe way to mutably access the wrapped value. The method will
    /// verify that the current thread matches the thread where the cell was created.
    ///
    /// # Panics
    ///
    /// Panics if called from a different thread than the one where this `SendCell`
    /// was created.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::new();
    /// let mut cell = SendCell::new(map);
    ///
    /// // Safe mutable access on the same thread
    /// cell.get_mut().insert("key", "value");
    /// assert_eq!(cell.get().get("key"), Some(&"value"));
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        assert_eq!(
            self.thread_id,
            crate::sys::thread::current().id(),
            "Access SendCell<{}> from incorrect thread",
            std::any::type_name::<T>()
        );
        unsafe { self.get_unchecked_mut() }
    }

    /// Unsafely consumes the cell and returns the wrapped value without thread checking.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - It is safe to take ownership of the value on the current thread
    /// - The value can be safely dropped on the current thread
    /// - No other references to the value exist
    ///
    /// This method bypasses the runtime thread check and may lead to undefined
    /// behavior if the wrapped type is not actually safe to move between threads.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    ///
    /// let cell = SendCell::new(42);
    ///
    /// // SAFETY: We're on the same thread, so this is safe
    /// let value = unsafe { cell.into_unchecked_inner() };
    /// assert_eq!(value, 42);
    /// ```
    #[inline]
    pub unsafe fn into_unchecked_inner(mut self) -> T {
        unsafe { self.inner.take().expect("gone").into_inner() }
    }
    /// Consumes the cell and returns the wrapped value with runtime thread checking.
    ///
    /// This is the safe way to extract the wrapped value from the cell. The method
    /// will verify that the current thread matches the thread where the cell was created.
    ///
    /// # Panics
    ///
    /// Panics if called from a different thread than the one where this `SendCell`
    /// was created.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::rc::Rc;
    ///
    /// let data = Rc::new("Hello, world!");
    /// let cell = SendCell::new(data);
    ///
    /// // Extract the original value
    /// let recovered_data = cell.into_inner();
    /// assert_eq!(*recovered_data, "Hello, world!");
    /// ```
    #[inline]
    pub fn into_inner(self) -> T {
        assert_eq!(self.thread_id, crate::sys::thread::current().id());
        unsafe { self.into_unchecked_inner() }
    }

    /// Creates a new cell with a different value, preserving the thread affinity.
    ///
    /// This creates a new `SendCell` that will be checked against the same thread
    /// as the original cell. This is useful for implementing clone/copy operations
    /// or transforming the wrapped value while maintaining thread safety.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The new value is safe to use on the same thread as the original cell
    /// - The new value can be safely dropped on the original thread
    /// - Any invariants expected by the new value are maintained
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::rc::Rc;
    ///
    /// let original = SendCell::new(Rc::new(42));
    ///
    /// // Create a new cell with a String on the same thread
    /// let derived = unsafe {
    ///     original.preserving_cell_thread("Hello".to_string())
    /// };
    ///
    /// assert_eq!(original.get().as_ref(), &42);
    /// assert_eq!(derived.get(), "Hello");
    /// ```
    #[inline]
    pub unsafe fn preserving_cell_thread<U>(&self, new: U) -> SendCell<U> {
        unsafe {
            SendCell {
                inner: Some(UnsafeSendCell::new_unchecked(new)),
                thread_id: self.thread_id,
            }
        }
    }

    /// Copies the wrapped value, creating a new cell on the same thread.
    ///
    /// This method is safe for types that implement `Copy` because copying
    /// such types doesn't involve custom code that could violate thread safety.
    /// The new cell will have the same thread affinity as the original.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    ///
    /// let original = SendCell::new(42i32);
    /// let copied = original.copying();
    ///
    /// assert_eq!(*original.get(), *copied.get());
    ///
    /// // They are independent cells
    /// std::mem::drop(original);
    /// assert_eq!(*copied.get(), 42);
    /// ```
    pub fn copying(&self) -> Self
    where
        T: Copy,
    {
        unsafe { self.preserving_cell_thread(*self.get_unchecked()) }
    }
}

impl<T: Future> SendCell<T> {
    /// Converts the cell into a future that implements Send with runtime thread checking.
    ///
    /// This method consumes the `SendCell` and returns a [`SendFuture`] that implements
    /// `Send` and can be moved between threads. However, the future will panic if polled
    /// from a different thread than the one where the original `SendCell` was created.
    ///
    /// Unlike [`crate::UnsafeSendCell::into_future()`], this provides memory safety
    /// through runtime checks.
    ///
    /// # Panics
    ///
    /// The returned future will panic if polled from a different thread than the one
    /// where this `SendCell` was created.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use send_cells::SendCell;
    /// use std::rc::Rc;
    ///
    /// // Create an async function that uses non-Send data
    /// async fn non_send_async() -> i32 {
    ///     let _local_data = Rc::new(42); // Not Send
    ///     42
    /// }
    ///
    /// let future = non_send_async();
    /// let cell = SendCell::new(future);
    /// let send_future = cell.into_future();
    ///
    /// // The future now implements Send
    /// fn assert_send<T: Send>(_: T) {}
    /// assert_send(send_future);
    /// ```
    pub fn into_future(mut self) -> SendFuture<T> {
        SendFuture {
            inner: self.inner.take().expect("inner value missing"),
            thread_id: self.thread_id,
        }
    }
}

impl<T> Drop for SendCell<T> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            assert_eq!(
                self.thread_id,
                crate::sys::thread::current().id(),
                "Drop SendCell<{}> from incorrect thread",
                std::any::type_name::<T>()
            );
        }
    }
}

// Trait implementations that delegate to the wrapped value
// All of these perform runtime thread checking through get() and get_mut()
impl<T: Debug> Debug for SendCell<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.get().fmt(f)
    }
}

impl<T> AsRef<T> for SendCell<T> {
    fn as_ref(&self) -> &T {
        self.get()
    }
}

impl<T> AsMut<T> for SendCell<T> {
    fn as_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}

impl<T> Deref for SendCell<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> DerefMut for SendCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

// Additional trait implementations
// For comparison traits (Eq, Hash, etc.), we rely on Deref to the underlying type
impl<T: Default> Default for SendCell<T> {
    fn default() -> SendCell<T> {
        SendCell::new(Default::default())
    }
}
impl<T> From<T> for SendCell<T> {
    fn from(value: T) -> Self {
        SendCell::new(value)
    }
}

/// A future wrapper that implements Send with runtime thread checking.
///
/// `SendFuture<T>` wraps a future of type `T` and provides a `Send` implementation
/// with runtime thread checking. The future remembers the thread it was created on
/// and panics if polled from any other thread.
///
/// This wrapper allows non-Send futures to be used in contexts that require Send futures
/// (such as being spawned on thread pools), while maintaining memory safety through
/// runtime checks. Unlike [`crate::UnsafeSendFuture`], this provides safe cross-thread
/// usage by panicking if accessed from the wrong thread.
///
/// # Examples
///
/// ```rust
/// use send_cells::SendCell;
/// use std::rc::Rc;
/// use std::future::Future;
/// use std::pin::Pin;
/// use std::task::{Context, Poll};
///
/// // A future that is not Send
/// struct NonSendFuture {
///     data: Rc<i32>,
/// }
///
/// impl Future for NonSendFuture {
///     type Output = i32;
///     
///     fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
///         Poll::Ready(*self.data)
///     }
/// }
///
/// let future = NonSendFuture { data: Rc::new(42) };
/// let cell = SendCell::new(future);
/// let send_future = cell.into_future();
///
/// // Now it can be used in Send contexts
/// fn requires_send_future<F: Future + Send>(_: F) {}
/// requires_send_future(send_future);
/// ```
///
/// # Panics
///
/// The `poll` method will panic if called from a different thread than the one
/// where the original `SendCell` was created.
#[derive(Debug)]
pub struct SendFuture<T> {
    inner: UnsafeSendCell<T>,
    thread_id: ThreadId,
}

// SAFETY: SendFuture implements Send by providing runtime thread checking.
// The wrapped future may not be Send, but we ensure safety by panicking
// if poll() is called from the wrong thread.
unsafe impl<T> Send for SendFuture<T> {}

impl<T: Future> Future for SendFuture<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Runtime thread check - panic if called from wrong thread
        assert_eq!(
            self.thread_id,
            crate::sys::thread::current().id(),
            "SendFuture<{}> polled from incorrect thread",
            std::any::type_name::<T>()
        );

        // SAFETY: After the thread check, we can safely access the inner future
        // using the same technique as UnsafeSendFuture
        let inner = unsafe {
            let self_mut = self.get_unchecked_mut();
            Pin::new_unchecked(self_mut.inner.get_mut())
        };
        inner.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    // A future that is NOT Send because it contains Rc<T>
    struct NonSendFuture {
        _data: Rc<i32>,
        ready: bool,
    }

    impl NonSendFuture {
        fn new(value: i32) -> Self {
            Self {
                _data: Rc::new(value),
                ready: false,
            }
        }
    }

    impl Future for NonSendFuture {
        type Output = i32;

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.ready {
                Poll::Ready(42)
            } else {
                self.ready = true;
                Poll::Pending
            }
        }
    }

    // Helper function to verify a type implements Send
    fn assert_send<T: Send>(_: &T) {}
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[test]
    fn test_send_cell_into_future_is_send() {
        // Create a non-Send future
        let non_send_future = NonSendFuture::new(42);

        // Wrap it in SendCell
        let cell = SendCell::new(non_send_future);

        // Convert to a Send future
        let send_future = cell.into_future();

        // Verify the resulting future is Send
        assert_send(&send_future);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[test]
    fn test_send_future_functionality() {
        // Create a no-op waker for testing
        static VTABLE: RawWakerVTable = RawWakerVTable::new(
            |_| RawWaker::new(std::ptr::null(), &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );
        let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
        let waker = unsafe { Waker::from_raw(raw_waker) };
        let mut context = Context::from_waker(&waker);

        // Create a non-Send future wrapped in SendCell
        let non_send_future = NonSendFuture::new(42);
        let cell = SendCell::new(non_send_future);
        let mut send_future = cell.into_future();

        // Test that the future still works correctly
        let pinned = Pin::new(&mut send_future);
        match pinned.poll(&mut context) {
            Poll::Pending => {
                // First poll should return Pending
                let pinned = Pin::new(&mut send_future);
                match pinned.poll(&mut context) {
                    Poll::Ready(value) => assert_eq!(value, 42),
                    Poll::Pending => panic!("Expected Ready on second poll"),
                }
            }
            Poll::Ready(value) => assert_eq!(value, 42),
        }
    }

    //no unwind on wasm!
    #[test]
    //at the moment, threads don't work in node: https://github.com/wasm-bindgen/wasm-bindgen/issues/4534
    fn test_send_future_cross_thread_panic() {
        use crate::sys::thread;
        use std::sync::{Arc, Mutex};

        // Create future on main thread
        let non_send_future = NonSendFuture::new(42);
        let cell = SendCell::new(non_send_future);
        let send_future = cell.into_future();

        // Share the future with another thread
        let future_mutex = Arc::new(Mutex::new(send_future));
        let future_clone = Arc::clone(&future_mutex);

        // Try to poll from a different thread - this should panic
        let handle = thread::spawn(move || {
            // Create a no-op waker inside the thread
            static VTABLE: RawWakerVTable = RawWakerVTable::new(
                |_| RawWaker::new(std::ptr::null(), &VTABLE),
                |_| {},
                |_| {},
                |_| {},
            );
            let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
            let waker = unsafe { Waker::from_raw(raw_waker) };
            let mut context = Context::from_waker(&waker);

            let mut future_guard = future_clone.lock().unwrap();
            let pinned = Pin::new(&mut *future_guard);
            let _ = pinned.poll(&mut context);
        });

        // Verify that the thread panicked
        let result = handle.join();
        assert!(
            result.is_err(),
            "Expected thread to panic when polling SendFuture from incorrect thread"
        );
    }
}
