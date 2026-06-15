use crate::arch::x86_64::interrupt::{disable_interrupts, enable_interrupts, interrupts_enabled};

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
        let was_enabled = interrupts_enabled();
        disable_interrupts();

        WithSpinLockGuard {
            inner: self.inner.lock(),
            was_enabled,
        }
    }
}

pub struct WithSpinLockGuard<'a, T> {
    inner: MutexGuard<'a, T>,
    was_enabled: bool,
}

impl<'a, T> Drop for WithSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            // Release the lock before restoring interrupts.
            drop_in_place(&mut self.inner);
            if self.was_enabled {
                enable_interrupts()
            }
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
