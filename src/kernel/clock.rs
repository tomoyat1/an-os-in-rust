pub(crate) enum Mode {
    UNINITIALIZED,
    ONESHOT,
    LOOP,
}

pub trait Clock {
    fn start_rate(&mut self);

    fn tick(&mut self);

    fn get_count(&self) -> u64;

    fn sleep(&self, milliseconds: u64) {
        let start = self.get_count();
        let end = start + milliseconds;
        while self.get_count() < end {
            unsafe {
                asm!("hlt");
            }
        }
    }
}
