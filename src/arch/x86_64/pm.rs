use alloc::{boxed, vec};

#[link(name = "pm")]
extern "C" {
    fn reload_gdt(gdtr: *const GDTR);
}

#[repr(C)]
#[repr(packed)]
struct GDTR {
    limit: u16,
    base: usize,
}

type GDT = vec::Vec<u64>;

struct SegmentDescriptor {
    base: usize,
    limit: u16,
    ty: u16,
}

impl From<SegmentDescriptor> for u64 {
    fn from(s: SegmentDescriptor) -> Self {
        let mut encoded: u64 = 0;
        let base = s.base as u64;
        let ty64 = s.ty as u64;
        encoded |= ((base & 0xffffff) << 16);
        encoded |= ((base & 0xff000000) << 32);
        encoded |= (ty64 << 40);

        encoded
    }
}

pub fn init_pm() -> GDT {
    let mut gdt = vec::Vec::with_capacity(8);
    unsafe { gdt.set_len(8) }
    let kernel_code = SegmentDescriptor {
        base: 0,
        limit: 0, // limits are ignored and not checked in IA-32e.
        ty: 0xaf9a,
    };
    let kernel_data = SegmentDescriptor {
        base: 0,
        limit: 0,
        ty: 0xcf92,
    };

    gdt[0] = 0x0;
    gdt[1] = kernel_code.into();
    gdt[2] = kernel_data.into();

    // Set GDTR
    let gdtr = GDTR {
        limit: 39,
        base: gdt.as_ptr() as usize,
    };
    unsafe {
        reload_gdt(&gdtr as *const GDTR);
    }

    // Return to caller so that our new GDT wouldn't get torn down.
    // maybe consider using Box::leak()
    gdt
}
