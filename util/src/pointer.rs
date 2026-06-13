use core::ops::Deref;

pub struct SyncMutPointer<T>(*mut T);
unsafe impl<T> Send for SyncMutPointer<T> {}
unsafe impl<T> Sync for SyncMutPointer<T> {}

impl<T> SyncMutPointer<T> {
    pub unsafe fn new(ptr: *mut T) -> Self {
        SyncMutPointer(ptr)
    }

    pub fn as_ptr(&self) -> *mut T {
        self.0
    }
}

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
