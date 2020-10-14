pub unsafe fn outb(port: u16, data: u8) {
    asm!(
        "out dx, al",
        in("rdx") port,
        in("rax") data as u16,
    )
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut val: u16 = 0;
    asm!(
        "in al, dx",
        in("rdx") port,
        out("rax") val,
    );
    val as u8
}

pub unsafe fn outl(port: u16, data: u32) {
    asm!(
    "out dx, eax",
    in("rdx") port,
    in("rax") data as u16,
    )
}

pub unsafe fn inl(port: u16) -> u32 {
    let mut val: u32 = 0;
    asm!(
    "in eax, dx",
    in("rdx") port,
    out("rax") val,
    );
    val
}
