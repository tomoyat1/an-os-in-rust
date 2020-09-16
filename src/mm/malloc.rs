use core::alloc;
use core::ptr::null_mut;

use crate::KERNEL_BASE;

#[link(name = "boot")]
extern "C" {
    // Don't use this for now
    static heap_bottom: usize;
}

#[global_allocator]
static mut KERNEL_ALLOCATOR: WithLock<KernelAllocator> = WithLock::<KernelAllocator>::new();

pub struct KernelAllocator {
    heap_bottom: usize,
    head: usize,
}

pub struct WithLock<A> {
    inner: spin::Mutex<A>
}

impl WithLock<KernelAllocator> {
    const fn new() -> Self {
        // Hard-code bottom of heap to KERNEL_BASE + 512MiB
        let bottom = KERNEL_BASE + (1 << 29);
        WithLock{
            inner: spin::Mutex::new(KernelAllocator{
                heap_bottom: bottom,
                head:  bottom,
            })
        }
    }
}

unsafe impl alloc::GlobalAlloc for WithLock<KernelAllocator> {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        let mut a = self.inner.lock();
        let mut head = a.head as usize;
        if head == 0 {
            return null_mut();
        }
        let (size, align) = (layout.size(), layout.align());
        head = (head / align) * align + align;
        let ptr = head as *mut u8;

        // TODO: consider OOM situations.
        // For now, blindly set head to (align + size)
        a.head = head + size;

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
    unsafe { panic!("oom") }
}
