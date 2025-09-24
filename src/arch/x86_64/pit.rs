use super::interrupt;
use super::port;
use crate::kernel::clock;
use crate::kernel::clock::Clock;
use crate::kernel::clocksource::ClockSource;
use crate::locking::spinlock::WithSpinLock;

const CHAN0_DATA: u8 = 0x40;
const CHAN1_DATA: u8 = 0x41;
const CHAN2_DATA: u8 = 0x42;
const MODE_CMD: u8 = 0x43;

const IOAPIC_LINE: u32 = 2;

static PIT: WithSpinLock<PIT> = WithSpinLock::new(PIT::new());

pub const TICK_INTERVAL: u64 = 1_000_000; // In nanoseconds

pub struct PIT {
    tick: Option<fn(u64)>,
}

impl PIT {
    pub const fn new() -> Self {
        Self { tick: None }
    }
}

pub fn register_tick(tick: fn(u64)) {
    let mut pid = PIT.lock();
    pid.tick = Some(tick);
}

// TODO: make this unsafe trait.
impl PIT {
    fn start_rate(&mut self) {
        // Use 1ms period for now.
        let count: u16 = 1193; // Close enough to 1ms period
        let control: u16 = 0b00110100; // chan 0; lobyte/hibyte; rate generator; 16-bit binary
        unsafe {
            port::outb(MODE_CMD as u16, control as u8);
            port::outb(CHAN0_DATA as u16, count as u8);
            port::outb(CHAN0_DATA as u16, (count >> 8) as u8);
        }
    }
}

pub fn pit_tick() {
    let pit = PIT.lock();
    if let Some(tick) = pit.tick {
        tick(TICK_INTERVAL);
    }
}
