//! Poison-safe lock accessors.
//!
//! `std::sync` locks become poisoned when a thread panics while holding them;
//! afterwards every `lock()` panics again, permanently breaking the subsystem
//! for the rest of the process lifetime. These helpers recover the guard
//! instead, so one stray panic cannot take down unrelated code.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Acquire a read guard, recovering from a poisoned lock.
pub fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}

/// Acquire a write guard, recovering from a poisoned lock.
pub fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

/// Acquire a mutex guard, recovering from a poisoned lock.
pub fn mutex_lock<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_recover_from_poisoned_locks() {
        let rw = RwLock::new(1u32);
        let _ = std::panic::catch_unwind(|| {
            let _guard = crate::sync_guard::write_lock(&rw);
            panic!("boom");
        });
        assert!(rw.is_poisoned());
        assert_eq!(*read_lock(&rw), 1);
        *write_lock(&rw) = 2;
        assert_eq!(*read_lock(&rw), 2);

        let m = Mutex::new(3u32);
        let _ = std::panic::catch_unwind(|| {
            let _guard = crate::sync_guard::mutex_lock(&m);
            panic!("boom");
        });
        assert_eq!(*mutex_lock(&m), 3);
    }
}
