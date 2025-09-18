pub trait ClockSource {
    fn get_tick(&self) -> u64;
}
