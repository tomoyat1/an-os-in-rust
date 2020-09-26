use core::ops::{Deref, DerefMut};
use core::ptr::drop_in_place;
use spin::MutexGuard;

pub struct WithSpinLock<A> {
    inner: spin::Mutex<A>,
}

impl<A> WithSpinLock<A> {
    pub const fn new(a: A) -> Self {
        Self {
            inner: spin::Mutex::new(a),
        }
    }

    pub fn lock(&self) -> WithSpinLockGuard<A> {
        // Disable interrupts to prevent deadlocks.
        unsafe { asm!("cli") };

        WithSpinLockGuard {
            inner: self.inner.lock(),
        }
    }
}

pub struct WithSpinLockGuard<'a, T> {
    inner: spin::MutexGuard<'a, T>,
}

impl<'a, T> Drop for WithSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            // self.inner should be dropped before sti
            drop_in_place(&mut self.inner as *mut MutexGuard<T>);
            asm!("sti");
        }
    }
}

impl<'a, T> Deref for WithSpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, T> DerefMut for WithSpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}
