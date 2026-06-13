use core::ops::Deref;

pub struct SyncMutPointer<T>(pub *mut T);
unsafe impl<T> Send for SyncMutPointer<T> {}
unsafe impl<T> Sync for SyncMutPointer<T> {}

impl<T> Deref for SyncMutPointer<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.0 }
    }
}

impl<T> From<&SyncMutPointer<T>> for usize {
    fn from(base: &SyncMutPointer<T>) -> usize {
        base.0 as usize
    }
}

impl<T> From<*mut T> for SyncMutPointer<T> {
    fn from(base: *mut T) -> SyncMutPointer<T> {
        SyncMutPointer(base)
    }
}
