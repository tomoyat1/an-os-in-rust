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
