use alloc::vec;

extern "C" {
    // fn reload_cs(gdtr: *const GDTR);
}

#[repr(C)]
#[repr(packed)]
struct GDTR {
    limit: u16,
    base: usize,
}

#[repr(C)]
#[repr(align(0x1000))]
pub struct GDT {
    data: [u64; 8192],
}

struct SegmentDescriptor {
    base: usize,
    limit: u16,
    ty: u8,
}

impl From<SegmentDescriptor> for u64 {
    fn from(s: SegmentDescriptor) -> Self {
        let mut encoded: u64 = 0;
        let base = s.base as u64;
        let ty64 = s.ty as u64;
        encoded |= (base & 0xffffff) << 16;
        encoded |= (base & 0xff000000) << 32;
        encoded |= ty64 << 40;

        encoded
    }
}

pub fn init_pm() -> GDT {
    let mut gdt = GDT {
        data: [0; 8192]
    };

    let kernel_code = SegmentDescriptor{
        base: 0,
        limit: 0, // limits are ignored and not checked in IA-32e.
        ty: 0x9A,
    };
    let kernel_data =  SegmentDescriptor{
        base: 0,
        limit: 0,
        ty: 0x92,
    };

    gdt.data[1] = kernel_code.into();
    gdt.data[2] = kernel_data.into();

    // Set GDTR
    let gdtr = GDTR{
        limit: 40,
        base: &gdt as *const GDT as usize
    };
    unsafe {
        // reload_cs(&gdtr as  *const GDTR);
    }

    // Return to caller so that our new GDT wouldn't get torn down.
    gdt
}
