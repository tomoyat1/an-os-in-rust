use super::interrupt;
use crate::kernel::clock;
use crate::kernel::clock::Clock;
use crate::locking::spinlock::WithSpinLock;

const CHAN0_DATA: u8 = 0x40;
const CHAN1_DATA: u8 = 0x41;
const CHAN2_DATA: u8 = 0x42;
const MODE_CMD: u8 = 0x43;

const IOAPIC_LINE: u32 = 2;

static mut PIT: WithSpinLock<PIT> = WithSpinLock::new(PIT::new());

pub struct PIT {
    count: u64,
    mode: clock::Mode,
}

impl PIT {
    pub const fn new() -> Self {
        Self { count: 0, mode: clock::Mode::UNINITIALIZED }
    }
}

pub unsafe fn start() -> impl Clock {
    let mut pit = unsafe{ PIT.lock()};
    pit.start_rate();
    interrupt::mask_line(false, IOAPIC_LINE);
    &PIT
}

// TODO: make this unsafe trait.
impl Clock for PIT {
    fn start_rate(&mut self) {
        // Use 1ms period for now.
        let count: u16 = 1193; // Close enough to 1ms period
        let control: u16 = 0b00110100; // chan 0; lobyte/hibyte; rate generator; 16-bit binary
        unsafe {
            asm!(
                "out {mode_cmd}, al",
                mode_cmd = const MODE_CMD,
                in("rax") control,
            );
            asm!(
                "out {chan0}, al",
                "mov al, ah",
                "out {chan0}, al",
                chan0 = const CHAN0_DATA,
                in("rax") count,
            )
        }
    }

    fn tick(&mut self) {
        self.count += 1;
    }

    fn get_count(&self) -> u64 {
        self.count
    }
}

impl Clock for &WithSpinLock<PIT> {
    fn start_rate(&mut self) {
        let mut pit = self.lock();
        pit.start_rate();
    }

    fn tick(&mut self) {
        let mut pit = self.lock();
        pit.tick();
    }

    fn get_count(&self) -> u64 {
        let pit = self.lock();
        pit.get_count()
    }
}

pub fn pit_tick() {
    let mut pit = unsafe { PIT.lock() };
    // increment clock count
    pit.tick();
}
