//! Fair mutex — prevents starvation of the PTY thread by the renderer.
//!
//! Adapted from Alacritty's `sync.rs`. A dual-mutex design where `lease()`
//! reserves the next lock slot, ensuring the PTY thread gets priority over
//! the renderer when both contend.

use parking_lot::{Mutex, MutexGuard};

/// A mutex with a fairness mechanism to prevent writer starvation.
///
/// The PTY reader thread calls `lease()` before blocking on I/O, reserving
/// the next lock slot. When data arrives, it calls `try_lock_unfair()` to
/// attempt a fast-path acquisition without waiting for the lease. If the
/// renderer holds the lock, the PTY thread accumulates more data until the
/// buffer is full, then forces a lock via `lock_unfair()`.
///
/// The renderer calls `lock()`, which first acquires the `next` mutex — so
/// if the PTY thread holds a lease, the renderer waits for it.
pub struct FairMutex<T> {
    data: Mutex<T>,
    next: Mutex<()>,
}

impl<T> FairMutex<T> {
    /// Create a new `FairMutex` wrapping `data`.
    pub fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Reserve the next lock slot (PTY thread calls this before reading).
    ///
    /// Hold the returned guard while reading from the PTY. When data arrives,
    /// the lease ensures the renderer cannot starve the PTY thread.
    pub fn lease(&self) -> MutexGuard<'_, ()> {
        self.next.lock()
    }

    /// Lock with fairness — waits for any lease holder first.
    ///
    /// The renderer uses this path so it yields to any pending PTY lock.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let _next = self.next.lock();
        self.data.lock()
    }

    /// Lock without fairness — bypasses the lease.
    ///
    /// Used by the PTY thread after a successful lease, or when the buffer
    /// is full and a forced lock is needed.
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Non-blocking lock attempt — returns `None` if the data mutex is held.
    ///
    /// PTY thread uses this as a fast path: if the renderer currently holds
    /// the lock, the PTY thread yields and accumulates more data.
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }
}
