pub struct WithSpinLock<A> {
    inner: spin::Mutex<A>
}

impl<A> WithSpinLock<A> {
    pub fn new(a: A) -> Self<A> {
        Self {
            inner: spin::Mutex::new(a),
        }
    }
}
