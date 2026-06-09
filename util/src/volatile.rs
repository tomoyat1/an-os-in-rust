use core::cell::SyncUnsafeCell;

#[repr(transparent)]
pub struct Volatile<T>(SyncUnsafeCell<T>);

impl<T: Copy> Volatile<T> {
    pub fn new(value: T) -> Self {
        Volatile(SyncUnsafeCell::new(value))
    }
    #[inline]
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(self.0.get()) }
    }

    #[inline]
    pub fn write(&self, value: T) {
        unsafe { core::ptr::write_volatile(self.0.get(), value) }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read() {
        let value = Volatile::new(1usize);
        assert_eq!(value.read(), 1);
    }

    #[test]
    fn test_write() {
        let value = Volatile::new(0usize);
        let alias = value.0.get();

        value.write(1);
        assert_eq!(unsafe { core::ptr::read_volatile(value.0.get()) }, 1);
    }
}
