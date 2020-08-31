use alloc::vec;

extern "C" {
    fn page_fault_isr();
    fn reload_idt(idtr: *const IDTR);
}

#[repr(C)]
#[repr(packed)]
struct IDTR {
    limit: u16,
    base: usize,
}

type IDT = vec::Vec<u128>;

pub fn init_int() {
    let mut idt = vec::Vec::<u128>::with_capacity(32);
    unsafe {
        idt.set_len(32);
    }

    // page fault handler
    {
        let mut descriptor: u128 = 0;
        let handler = page_fault_isr as usize;
        descriptor |= (handler & 0xffff) as u128; // offset 15:0
        descriptor |= ((handler & 0xffffffffffff0000) as u128) << 32; // offset 63:16
        descriptor |= 0x8 << 16; // segment selector
        descriptor |= 0xe << 40; // type: 0xb1110
        descriptor |= 8 << 44; // Present flag

        idt[14] = descriptor;
    }

    // Set IDTR
    let idtr = IDTR {
        limit: 255, // 32 * 8 - 1
        base: idt.as_ptr() as usize,
    };
    unsafe {
        reload_idt(&idtr as *const IDTR);
    }

    // dereference null ptr for shits and giggles
    let foo: *mut u64 = 0x0 as *mut u64;
    unsafe {
        // This should page fault
        *foo = 0xdeadbeef;
    }
}

#[no_mangle]
unsafe extern "C" fn page_fault_handler() {
    let foo = 1 + 1;
    /* no-op */
}
