/// This module contains data types passed to the kernel.
use uefi::table;
use uefi::table::boot;

/// BootData is the data structure passed to the kernel.
#[repr(C)]
pub struct BootData {
    /// Identity-mapped base address of MemoryDescriptor array.
    pub memory_map_buf: *mut boot::MemoryDescriptor,

    /// Number of MemoryDescriptors in array.
    pub memory_map_len: usize,
    pub framebuffer: RawFramebuffer,
    pub system_table: *const table::SystemTable<table::Runtime>,
}

#[repr(C)]
pub struct RawFramebuffer {
    pub framebuffer_base: *mut u8,
    pub framebuffer_size: usize,
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub pixels_per_scan_line: usize,
    pub pixel_format: uefi::proto::console::gop::PixelFormat,
}
