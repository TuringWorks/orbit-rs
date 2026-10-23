//! Poison-tolerant access to [`std::sync`] locks.
//!
//! A `std` lock is *poisoned* when a thread panics while holding its guard. The
//! protected data is still memory-safe; poisoning only signals that an invariant
//! may have been interrupted mid-update. For the caches, registries and counters
//! guarded in Orbit, continuing with the last written state is preferable to
//! cascading the panic into every other thread that touches the lock.
//!
//! Use these methods instead of `lock().unwrap()` so that policy is stated once.
//! Where a half-applied update would be unsound, handle [`std::sync::PoisonError`]
//! explicitly instead.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

mod sealed {
    pub trait Sealed {}
    impl<T: ?Sized> Sealed for std::sync::Mutex<T> {}
    impl<T: ?Sized> Sealed for std::sync::RwLock<T> {}
}

/// Poison-recovering accessors for [`Mutex`].
pub trait MutexExt<T: ?Sized>: sealed::Sealed {
    /// Acquires the mutex, returning the guard even if the lock was poisoned.
    #[must_use = "dropping the guard releases the lock immediately"]
    fn lock_recover(&self) -> MutexGuard<'_, T>;
}

/// Poison-recovering accessors for [`RwLock`].
pub trait RwLockExt<T: ?Sized>: sealed::Sealed {
    /// Acquires shared access, returning the guard even if the lock was poisoned.
    #[must_use = "dropping the guard releases the lock immediately"]
    fn read_recover(&self) -> RwLockReadGuard<'_, T>;

    /// Acquires exclusive access, returning the guard even if the lock was poisoned.
    #[must_use = "dropping the guard releases the lock immediately"]
    fn write_recover(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T: ?Sized> MutexExt<T> for Mutex<T> {
    fn lock_recover(&self) -> MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<T: ?Sized> RwLockExt<T> for RwLock<T> {
    fn read_recover(&self) -> RwLockReadGuard<'_, T> {
        self.read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_recover(&self) -> RwLockWriteGuard<'_, T> {
        self.write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn poison_mutex() -> Arc<Mutex<u32>> {
        let m = Arc::new(Mutex::new(7));
        let m2 = Arc::clone(&m);
        let _ = std::thread::spawn(move || {
            let _guard = m2.lock_recover();
            panic!("poison");
        })
        .join();
        assert!(m.is_poisoned());
        m
    }

    #[test]
    fn mutex_recovers_after_poisoning() {
        assert_eq!(*poison_mutex().lock_recover(), 7);
    }

    #[test]
    fn rwlock_recovers_after_poisoning() {
        let l = Arc::new(RwLock::new(1u32));
        let l2 = Arc::clone(&l);
        let _ = std::thread::spawn(move || {
            let _guard = l2.write_recover();
            panic!("poison");
        })
        .join();
        assert!(l.is_poisoned());
        *l.write_recover() += 1;
        assert_eq!(*l.read_recover(), 2);
    }
}
