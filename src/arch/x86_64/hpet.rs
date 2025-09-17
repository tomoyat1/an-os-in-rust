use alloc::string::String;

use crate::arch::x86_64::interrupt;
use crate::arch::x86_64::interrupt::LOCAL_APIC;
use crate::drivers::acpi;
use crate::locking::spinlock::{WithSpinLock, WithSpinLockGuard};
use core::arch::asm;
use core::mem::{offset_of, MaybeUninit};
use core::ptr;

const REG_GENERAL_CAPABILITY: usize = 0x0;
const REG_GENERAL_CONFIGURATION: usize = 0x10;
const REG_MAIN_COUNTER: usize = 0xf0;
const REG_TIMER_N_CONFIG_CAPABILITY: usize = 0x100;

static HPET: WithSpinLock<Option<HPET>> = WithSpinLock::new(None);

pub struct HPET {
    base_address: usize,
    pub comparator_count: u8,
    pub counter_size: u8,
    pub legacy_replacement_irq_routing_capable: bool,

    // Timer period in nanoseconds.
    pub counter_clock_period: u32,

    last_tick_count_timer_0: u64,

    // Function to call on each tick.
    tick: Option<fn(u64)>,
}

impl HPET {
    fn from_acpi(hpet: acpi::HPET) -> Self {
        let mut hpet = HPET {
            base_address: hpet.base_address,
            comparator_count: hpet.comparator_count,
            counter_size: hpet.counter_size,
            legacy_replacement_irq_routing_capable: hpet.legacy_replacement_irq_routing_capable,
            counter_clock_period: 0,
            tick: None,
            last_tick_count_timer_0: 0,
        };

        // Safety: REG_GENCAP is 32-bit aligned.
        let capabilities = unsafe { hpet.inw(REG_GENERAL_CAPABILITY) };
        assert_eq!(hpet.comparator_count, ((capabilities >> 8) & 0x1f) as u8);
        assert_eq!(hpet.counter_size, ((capabilities >> 13) & 0x1) as u8);
        assert_eq!(
            hpet.legacy_replacement_irq_routing_capable,
            ((capabilities >> 15) & 0x1) != 0
        );

        // Safety: REG_GENCAP is 32-bit aligned, and adding 4 bytes to that is also 32-bit aligned.
        let period_femto_seconds = unsafe { hpet.inw(REG_GENERAL_CAPABILITY + 4) };
        hpet.counter_clock_period = period_femto_seconds / 1_000_000;

        hpet
    }

    /// Enable the timer by setting ENABLE_CNF.
    fn enable(&mut self) {
        // Safety: REG_GENCNF is 32-bit aligned.
        let mut gen_cnf = unsafe { self.inw(REG_GENERAL_CONFIGURATION) };
        gen_cnf |= 0x1;
        // Safety: REG_GENCNF is 32-bit aligned.
        unsafe { self.outw(REG_GENERAL_CONFIGURATION, gen_cnf) };
    }

    // Read the current main counter value.
    fn read_main_counter(&self) -> u64 {
        // Safety: REG_MAIN_COUNTER is 64-bit aligned.
        unsafe { self.ing(REG_MAIN_COUNTER) }
    }

    // Resets the main counter to 0.
    fn reset_main_counter(&mut self) {
        // Safety: REG_MAIN_COUNTER is 64-bit aligned.
        unsafe { self.outg(REG_MAIN_COUNTER, 0) }
    }

    fn get_timer_config(&self, timer: usize) -> u32 {
        if self.comparator_count == 0 {
            return 0;
        }
        let offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20;

        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 32-bit aligned.
        unsafe { self.inw(offset) }
    }

    fn set_timer_config(&mut self, timer: usize, config: u32) {
        if timer >= self.comparator_count as usize {
            return;
        }
        let offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20;

        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 32-bit aligned.
        unsafe { self.outw(offset, config) }
    }

    /// Returns the ISAs that the timer's interrupts can be routed to.
    fn timer_routing_capability(&self, timer: usize) -> u32 {
        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 32-bit aligned.
        if timer >= self.comparator_count as usize {
            return 0;
        }
        let offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20 + 0x4;
        unsafe { self.inw(offset) }
    }

    fn get_timer_comparator(&self, timer: usize) -> u64 {
        let offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20 + 0x8;
        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 64-bit aligned.
        unsafe { self.ing(offset) }
    }

    fn set_timer_comparator(&mut self, timer: usize, count: u64) {
        let cap_cnf_offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20 + 0x8;
        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 32-bit aligned.
        let mut cap_cnf = unsafe { self.inw(cap_cnf_offset) };
        cap_cnf |= (0x1 << 6);
        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 32-bit aligned.
        unsafe { self.outw(cap_cnf_offset, cap_cnf) };

        let comparator_offset = REG_TIMER_N_CONFIG_CAPABILITY + timer * 0x20 + 0x8;
        // Safety: REG_TIMER_N_CONFIG_CAPABILITY is 64-bit aligned.
        unsafe { self.outg(comparator_offset, count) }
    }

    /// Read a 32-bit value from a memory mapped register.
    // Safety: The base address is read from ACPI tables and is a valid address.
    //         Caller must ensure that `offset` is a 32-bit aligned offset that is mapped to
    //         an HPET register.
    unsafe fn inw(&self, offset: usize) -> u32 {
        let addr = (self.base_address + offset) as *const u32;
        ptr::read_volatile(addr)
    }
    /// Read a 64-bit value from a memory mapped register.
    // Safety: The base address is read from ACPI tables and is a valid address.
    //         Caller must ensure that `offset` is a 64-bit aligned offset that is mapped to
    //         an HPET register.
    unsafe fn ing(&self, offset: usize) -> u64 {
        let addr = (self.base_address + offset) as *const u64;
        ptr::read_volatile(addr)
    }

    /// Write a 32-bit value to a memory mapped register.
    // Safety: The base address is read from ACPI tables and is a valid address.
    //         Caller must ensure that `offset` is a 32-bit aligned offset that is mapped to
    //         an HPET register.
    unsafe fn outw(&mut self, offset: usize, value: u32) {
        let addr = (self.base_address + offset) as *mut u32;
        ptr::write_volatile(addr, value);
    }

    /// Write a 64-bit value to a memory mapped register.
    // Safety: The base address is read from ACPI tables and is a valid address.
    //         Caller must ensure that `offset` is a 64-bit aligned offset that is mapped to
    //         an HPET register.
    unsafe fn outg(&mut self, offset: usize, value: u64) {
        let addr = (self.base_address + offset) as *mut u64;
        ptr::write_volatile(addr, value);
    }
}

pub fn init(hpet: acpi::HPET) {
    let mut hpet = HPET::from_acpi(hpet);
    let timer_0_routing_cap = hpet.timer_routing_capability(0);

    // Panic if we cannot route the timer interrupt to ISA line 2.
    // TODO: pick a valid line and configure interrupts dynamically.
    assert_ne!(timer_0_routing_cap >> 2 & 0x1, 0);
    let mut timer_0_config = hpet.get_timer_config(0);

    // Route interrupts to ISA line 5.
    timer_0_config |= (0x5 << 9);

    // Periodic mode
    // Panic if the timer doesn't support periodic mode.
    // TODO: pick a timer that supports periodic mode.
    assert_eq!((timer_0_config >> 4) & 0x1, 1);
    timer_0_config |= (0x1 << 3);

    // Enable interrupts
    timer_0_config |= (0x1 << 2);

    // Edge triggered
    timer_0_config |= (0x0);

    hpet.set_timer_config(0, timer_0_config);
    // Assume that the clock period is valid
    // TODO: gracefully fail if not so.
    assert_ne!(hpet.counter_clock_period, 0);
    // 1 millisecond
    hpet.set_timer_comparator(0, 1_000_000 / (hpet.counter_clock_period) as u64); // 1,000,000 ns / (10ns / tick)

    hpet.enable();
    interrupt::mask_line(false, 2);

    HPET.lock().replace(hpet);
}

/// Get time in nanoseconds since when the main timer was started.
pub fn get_time() -> u64 {
    match HPET.lock().as_mut() {
        Some(hpet) => {
            let ticks = hpet.read_main_counter();
            ticks * hpet.counter_clock_period as u64
        }
        None => 0
    }
}

pub fn register_tick(tick: fn(u64)) {
    let mut hpet = HPET.lock();
    if let Some(hpet) = hpet.as_mut() {
        hpet.tick = Some(tick);
    }
}

pub fn hpet_tick() {
    let mut hpet = HPET.lock();
    if let Some(hpet) = hpet.as_mut() {
        let current_count = hpet.read_main_counter(); // tick/10ns
        let delta_ticks = current_count - hpet.last_tick_count_timer_0; // tick/10ns
        if let Some(tick) = hpet.tick {
            tick(delta_ticks * hpet.counter_clock_period as u64);
        }
        hpet.last_tick_count_timer_0 = current_count;
    }
}
