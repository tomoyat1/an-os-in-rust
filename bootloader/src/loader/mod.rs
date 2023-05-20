use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CStr16;

pub mod elf;

pub fn load_file(system_table: &SystemTable<Boot>) -> core::result::Result<Vec<u8>, String> {
    let bs = system_table.boot_services();
    let fs = bs.locate_protocol::<SimpleFileSystem>().map_err(|e| {
        String::from(format!(
            "Simple File System Protocol support is required: {:?}",
            e.status()
        ))
    })?;
    let fs = unsafe { &mut *fs.get() };
    let mut kernel_obj = [0; 8];
    let kernel_obj = CStr16::from_str_with_buf("aosir", &mut kernel_obj);
    let dir = &mut fs
        .open_volume()
        .map_err(|e| String::from(format!("failed to open root directory: {:?}", e.status())))?;
    let kernel_file = dir
        .open(
            kernel_obj.unwrap(),
            FileMode::Read,
            FileAttribute::READ_ONLY,
        )
        .map_err(|e| String::from(format!("failed to obtain file handle: {:?}", e.status())))?;
    let kernel_file = kernel_file
        .into_type()
        .map_err(|e| String::from(format!("failed to get file type: {:?}", e.status())))?;
    match kernel_file {
        FileType::Dir(_) => Err(String::from(
            "directory found instead of kernel binary at /aosir",
        )),
        FileType::Regular(mut f) => {
            let mut info_buf = Vec::with_capacity(1);
            unsafe {
                info_buf.set_len(1);
            }
            let size = f
                .get_info::<FileInfo>(&mut info_buf)
                .expect_err("file info size is 1 byte :thinking_face:");
            if let (Some(size), Status::BUFFER_TOO_SMALL) = (size.data(), size.status()) {
                info_buf.resize(*size, 0);
                let info = f.get_info::<FileInfo>(&mut info_buf).map_err(|e| {
                    String::from(format!("failed to get file info: {:?}", e.status()))
                })?;
                info.file_size();
                let mut buf: Vec<u8> = Vec::with_capacity(info.file_size() as usize);
                unsafe {
                    buf.set_len(info.file_size() as usize);
                }
                let read_size = f.read(&mut buf).map_err(|e| {
                    String::from(format!("failed to read kernel: {:?}", e.status()))
                })?;
                Ok(buf)
            } else {
                Err(format!(
                    "unexpected error in obtaining file info: {:?}",
                    size.status()
                ))
            }
        }
    }
}
