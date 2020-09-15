pub struct WithSpinLock<A> {
    inner: spin::Mutex<A>
}

impl<A> WithSpinLock<A> {
    pub const fn new(a: A) -> Self {
        Self {
            inner: spin::Mutex::new(a),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
