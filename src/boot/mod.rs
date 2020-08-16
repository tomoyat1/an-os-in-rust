use uefi::table;
use uefi::table::boot;

use bootloader::boot_types;
use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use uefi::table::boot::MemoryDescriptor;
use core::borrow::Borrow;
use uefi::prelude::SystemTable;

pub(crate) struct BootData<'a> {
    pub memory_map: &'a [boot::MemoryDescriptor],
    pub framebuffer: RawFramebuffer<'a>,
    pub system_table: &'a table::SystemTable<table::Runtime>
}

impl<'a> BootData<'a> {
    pub fn relocate(mut phys_boot_data: &mut boot_types::BootData, kernel_base: usize) -> Self {
        let mm_sz = phys_boot_data.memory_map.len();
        let mm_ptr = phys_boot_data.memory_map.as_ptr() as usize + kernel_base;
        let mm_ptr = mm_ptr as *const MemoryDescriptor;
        let mmap = slice_from_raw_parts(mm_ptr as *const MemoryDescriptor, mm_sz);

        let st_ptr = phys_boot_data.system_table as *const SystemTable<table::Runtime>;
        let st_ptr = (st_ptr as usize + kernel_base) as *const SystemTable<table::Runtime>;
        Self {
            memory_map: unsafe { &*mmap },
            framebuffer: RawFramebuffer::relocate(&phys_boot_data.framebuffer, kernel_base),
            system_table: unsafe {&*st_ptr},
        }
    }
}

pub(crate) struct RawFramebuffer<'a> {
    pub framebuffer: &'a mut [u8],
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub pixels_per_scan_line: usize,
    pub pixel_format: uefi::proto::console::gop::PixelFormat,
}

impl<'a> RawFramebuffer<'a> {
    fn relocate(phys_fb: &boot_types::RawFramebuffer, kernel_base: usize) -> Self {
        let fb_ptr = phys_fb.framebuffer_base;
        let fb_ptr = (fb_ptr as usize + kernel_base) as *mut u8;
        let fb_sz = phys_fb.framebuffer_size;
        let fb_buf = slice_from_raw_parts_mut(fb_ptr, fb_sz);
        Self {
            framebuffer: unsafe {& mut *fb_buf},
            horizontal_resolution: phys_fb.horizontal_resolution,
            vertical_resolution: phys_fb.vertical_resolution,
            pixels_per_scan_line: phys_fb.pixels_per_scan_line,
            pixel_format: phys_fb.pixel_format,
        }
    }
}
