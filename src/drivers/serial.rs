use crate::arch::x86_64::port;
use crate::locking::spinlock::WithSpinLock;

const COM1_PORT: u16 = 0x3f8;

static mut COM1: WithSpinLock<Option<Com>> = WithSpinLock::new(None);

pub fn init() {
    let divisor: u16 = 0x2;

    let com = Com::new(COM1_PORT, divisor);
    unsafe {
        let mut com1 = COM1.lock();
        *com1 = Some(com);
    }
}

pub fn tmp_write_com1(c: u8) {
    let com1 = unsafe {
        COM1.lock()
    };
    let com1 = com1.as_ref();
    match com1 {
        Some(com1) => {
            while com1.line_status() & 0b01000000 == 0 {
                let foo = 1 + 1;
            }
            com1.outb(0, c);
        },
        None => {}

    }
}

pub fn read_com1() {
    unsafe {
        let com1 = COM1.lock();
        let com1 = com1.as_ref();
        match com1 {
            Some(com1) => {
                while com1.line_status() & 0x1 == 1 {
                    let data = com1.inb(0);
                    com1.outb(0, data as u8)
                }
            },
            None => {}
        }
    }
}

pub struct Com {
    port: u16,
}

impl Com {
    fn new(port: u16, divisor: u16) -> Self {
        // set divisor
        let com = Self {port};
        unsafe {
            com.outb(1, 0);
            com.outb(3, 0x80);
            com.outb(0, divisor as u8);
            com.outb(1, (divisor >> 8) as u8);
            com.outb(3, 0x03);

            // Enable FIFO, clear them, with 14-byte threshold?
            com.outb(2, 0xc7);

            // Enable IRQs, RTS/DTS set?
            com.outb(1, 0b00000111);
            com.outb(4, 0x0b);
        }

        com
    }

    fn inb(&self, offset: u16) -> u16 {
        unsafe {
            port::inb(self.port + offset) as u16
        }
    }

    pub fn outb(&self, offset: u16, data: u8) {
        unsafe {
            port::outb(self.port + offset, data);
        }
    }

    fn line_status(&self) -> u16 {
        self.inb(5)
    }
}
