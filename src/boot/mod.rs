use core::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut};
use core::ffi::c_void;

use uefi::table;
use uefi::table::boot;
use uefi::table::boot::MemoryDescriptor;

pub(crate) struct BootData<'a> {
    pub memory_map: &'a [boot::MemoryDescriptor],
    pub framebuffer: RawFramebuffer<'a>,
    pub system_table: &'a table::SystemTable<table::Runtime>,
    pub acpi_rsdp:  *const c_void,
}

impl<'a> BootData<'a> {
    pub fn relocate(mut phys_boot_data: *mut bootlib::types::BootData, kernel_base: usize) -> Self {
        let phys_boot_data = unsafe { &mut *phys_boot_data };
        let mm_sz = phys_boot_data.memory_map_len;
        let acpi_rsdp = phys_boot_data.acpi_rsdp;

        // On QEMU, phys_boot_data.memory_map_buf happens to be larger than 2 GiB, causing mm_ptr to
        // overflow and resulting in a panic. Oh, shit.
        let mm_ptr = phys_boot_data.memory_map_buf;
        let mmap = slice_from_raw_parts(mm_ptr as *const MemoryDescriptor, mm_sz);

        Self {
            memory_map: unsafe { &*mmap },
            framebuffer: RawFramebuffer::relocate(&phys_boot_data.framebuffer, kernel_base),
            system_table: unsafe { &*phys_boot_data.system_table },
            acpi_rsdp,
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
    fn relocate(phys_fb: &bootlib::types::RawFramebuffer, kernel_base: usize) -> Self {
        let fb_ptr = phys_fb.framebuffer_base;
        let fb_sz = phys_fb.framebuffer_size;
        let fb_buf = slice_from_raw_parts_mut(fb_ptr, fb_sz);
        Self {
            framebuffer: unsafe { &mut *fb_buf },
            horizontal_resolution: phys_fb.horizontal_resolution,
            vertical_resolution: phys_fb.vertical_resolution,
            pixels_per_scan_line: phys_fb.pixels_per_scan_line,
            pixel_format: phys_fb.pixel_format,
        }
    }
}
