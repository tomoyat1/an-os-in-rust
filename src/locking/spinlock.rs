use core::arch::asm;
use core::cell::SyncUnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::drop_in_place;
use spin::MutexGuard;

use crate::kernel::sched;

pub struct WithSpinLock<A> {
    inner: SyncUnsafeCell<spin::Mutex<A>>,
}

impl<A> WithSpinLock<A> {
    pub const fn new(a: A) -> Self {
        Self {
            inner: SyncUnsafeCell::new(spin::Mutex::new(a)),
        }
    }

    pub fn lock(&self) -> WithSpinLockGuard<A> {
        // Disable interrupts to prevent deadlocks.
        let rflags: u64;
        unsafe {
            asm!(
            "pushfq",
            "pop {reg}",
            reg = out(reg) rflags
            )
        }
        let interrupt_enabled = (rflags >> 9) & 1 != 0;
        if interrupt_enabled {
            unsafe { asm!("cli") };
        }

        // SAFETY: Access exclusivity is guaranteed through the spinlock Mutex.
        let spin_mutex = unsafe { &mut *self.inner.get() };
        let guard = spin_mutex.lock();

        WithSpinLockGuard {
            inner: guard,
            interrupt_enabled,
        }
    }
}

pub struct WithSpinLockGuard<'a, T> {
    inner: MutexGuard<'a, T>,
    interrupt_enabled: bool,
}

impl<'a, T> Drop for WithSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            // self.inner should be dropped before sti
            drop_in_place(&mut self.inner);
            if self.interrupt_enabled {
                asm!("sti");
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
