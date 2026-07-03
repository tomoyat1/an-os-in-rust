use core::arch::asm;

pub unsafe fn outb(port: u16, data: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("rdx") port,
            in("rax") data as u16,
        )
    }
}

pub unsafe fn inb(port: u16) -> u8 {
    unsafe {
        let mut val: u16 = 0;
        asm!(
            "in al, dx",
            in("rdx") port,
            out("rax") val,
        );
        val as u8
    }
}

pub unsafe fn outw(port: u16, data: u16) {
    unsafe {
        asm!(
        "out dx, ax",
        in("rdx") port,
        in("rax") data as u16,
        )
    }
}

pub unsafe fn inw(port: u16) -> u16 {
    unsafe {
        let mut val: u16 = 0;
        asm!(
        "in ax, dx",
        in("rdx") port,
        out("rax") val,
        );
        val as u16
    }
}

pub unsafe fn outl(port: u16, data: u32) {
    unsafe {
        asm!(
        "out dx, eax",
        in("rdx") port,
        in("rax") data as u32,
        )
    }
}

pub unsafe fn inl(port: u16) -> u32 {
    unsafe {
        let mut val: u32 = 0;
        asm!(
        "in eax, dx",
        in("rdx") port,
        out("rax") val,
        );
        val
    }
}
