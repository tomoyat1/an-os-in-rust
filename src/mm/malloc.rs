use core::alloc;
use core::ptr::null_mut;

extern "C" {
    #[link_name = "heap_bottom"]
    static mut HEAP_BOTTOM: usize;
}

#[global_allocator]
static mut KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator {
    heap_bottom: 0,
    head: 0,
};

pub struct KernelAllocator {
    heap_bottom: usize,
    head: usize,
}

pub fn init() {
    unsafe {
        KERNEL_ALLOCATOR.heap_bottom = HEAP_BOTTOM;
        KERNEL_ALLOCATOR.head = HEAP_BOTTOM;
    }
}

unsafe impl alloc::GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        let mut head = self.head as usize;
        if head == 0 {
            return null_mut()
        }
        let (size, align) = (layout.size(), layout.align());
        head = (head / align) * align + align;
        let ptr = head as *mut u8;

        // TODO: consider OOM situations.
        // For now, blindly set head to (align + size)
        KERNEL_ALLOCATOR.head = head + size;

        ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: alloc::Layout) {
        // We leak memory, it's just how we do things.
        ()
    }
}

#[alloc_error_handler]
#[no_mangle]
fn oom(_: alloc::Layout) -> ! {
    unsafe {
        panic!("oom")
    }
}
