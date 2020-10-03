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

pub fn tmp_write_com1(buf: &[u8]) {
    let com1 = unsafe { COM1.lock() };
    let com1 = com1.as_ref();
    match com1 {
        Some(com1) => {
            com1.write_all(buf);
        }
        None => {}
    }
}

pub fn read_com1() {
    let mut buf: [u8; 16] = [0; 16];
    unsafe {
        let com1 = COM1.lock();
        let com1 = com1.as_ref();
        match com1 {
            Some(com1) => {
                    if let Ok(len) = com1.read(&mut buf) {
                        // TODO: read byte into the driver's buffer instead of writing it out.
                        //       deciding to write the byte out should be the job of whoever gets the
                        //       byte from the buffer.
                        let buf = &buf[0..len];
                        com1.write_all(buf);
                    }
            }
            None => {}
        }
    }
}

pub struct Com {
    port: u16,
    // TODO: Add read and write buffers. Keep in mind that these will be accessed in IRQ context, so
    //       task switching while manipulating them is a big no-no.
}

impl Com {
    fn new(port: u16, divisor: u16) -> Self {
        // set divisor
        let com = Self { port };
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

    fn inb(&self, offset: u16) -> u8 {
        unsafe { port::inb(self.port + offset) }
    }

    fn outb(&self, offset: u16, data: u8) {
        unsafe {
            port::outb(self.port + offset, data);
        }
    }

    fn line_status(&self) -> u8 {
        self.inb(5)
    }

    /// Writes `byte` to serial port `self`
    ///
    /// ## TODO
    /// - This should be abstracted as a character device down the road.
    /// - Add a buffer to store bytes until port is ready for more data. Drop further outgoing data
    ///   if this buffer is full.
    fn write_byte(&self, byte: u8) {
        self.outb(0, byte);
    }

    /// Reads a byte from serial port `self`
    ///
    /// ## TODO
    /// - This should be abstracted as a character device down the road.
    /// - Add a buffer to store bytes until read by this methods. Drop further incoming data if this
    ///   buffer is full.
    fn read_byte(&self) -> u8 {
        self.inb(0) as u8
    }

    /// Reads bytes from serial port `self` into `buf`. This method reads what is available
    /// in the serial ports internal buffer, and does not block. This method returns the number of
    /// bytes read, or an error value if something goes wrong.
    ///
    /// # Errors
    /// An value of core::result::Result::Err(()) will be returned if an unexpected error is encountered.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut len: usize = 0;
        for b in buf.iter_mut() {
            if self.line_status() & 0x1 != 1 {
                break;
            }
            *b = self.read_byte();
            len += 1;
        }
        Ok(len)
    }

    /// Writes bytes from `buf` into serial port `self`. This method will block until all the data
    /// in `buf` is written to the serial port. This method returns the number of
    /// bytes written, or an error value if something goes wrong.
    ///
    /// # Errors
    /// An value of core::result::Result::Err(()) will be returned if an unexpected error is encountered.
    pub fn write(&self, buf: &[u8]) -> Result<usize, ()> {
        let mut len: usize = 0;
        for b in buf {
            if self.line_status() & 0b100000 != 0b100000 {
                break;
            }
            self.write_byte(*b);
            len += 1;
        }
        Ok(len)
    }

    /// Writes all of the bytes in `buf` to serial port `self`. This method will block and wait if
    /// necessary.
    pub fn write_all(&self, buf: &[u8]) -> Result<(), ()> {
        let mut slice = buf;
        while slice.len() > 0 {
            match self.write(slice) {
                Ok(written) => {
                    let (_, t) = slice.split_at(written);
                    slice = t;
                },
                Err(_) => return Err(())
            };
        };
        Ok(())
    }
}
