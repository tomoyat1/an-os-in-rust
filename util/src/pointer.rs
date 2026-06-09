pub struct SyncMutPointer<T>(pub *mut T);
unsafe impl<T> Send for SyncMutPointer<T> {}
unsafe impl<T> Sync for SyncMutPointer<T> {}

impl<T> From<&SyncMutPointer<T>> for usize {
    fn from(base: &SyncMutPointer<T>) -> usize {
        base.0 as usize
    }
}
