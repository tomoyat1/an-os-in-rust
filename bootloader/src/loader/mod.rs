use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileMode, FileInfo, FileType, FileAttribute};
use uefi::proto::media::fs::SimpleFileSystem;

pub mod elf;

pub fn load_file(system_table: &SystemTable<Boot>) -> core::result::Result<usize, String>{
    let bs = system_table.boot_services();
    let fs = bs.locate_protocol::<SimpleFileSystem>()
        .expect("Simple File System Protocol support is required!");
    let fs = fs.expect("warnings occurred when opening SFS");
    let fs = unsafe { &mut *fs.get()};
    let dir = &mut fs.open_volume()
        .expect("failed to open root directory")
        .expect("warnings occurred when opening directory");
    let kernel_file = dir.open("aosir", FileMode::Read, FileAttribute::READ_ONLY)
        .expect("failed to obtain file handle")
        .expect("warnings occurred when obtaining file handle");
    let kernel_file_typed = kernel_file.into_type()
        .expect("failed to get file type")
        .expect("warnings occurred when getting file type");
    match kernel_file_typed {
        FileType::Dir(_) => {
            Err(String::from("directory found instead of kernel binary at /aosir"))
        },
        FileType::Regular(mut f) => {
            let mut info_buf = Vec::with_capacity(1);
            unsafe {
                info_buf.set_len(1);
            }
            let size = f.get_info::<FileInfo>(&mut info_buf)
                .expect_err("file info size is 1 byte :thinking_face:");
            if let (Some(size), Status::BUFFER_TOO_SMALL) = (size.data(), size.status()) {
                info_buf.resize(*size, 0);
                let info = f.get_info::<FileInfo>(&mut info_buf)
                    .expect("failed to get file info")
                    .expect("warnings when getting file info");
                info.file_size();
                let mut buf: Vec<u8> = Vec::with_capacity(info.file_size() as usize);
                unsafe {
                    buf.set_len(info.file_size() as usize);
                }
                let read_size = f.read(&mut buf)
                    .expect("failed to read kernel")
                    .expect("warnings when reading kernel");
                Ok(read_size)
            } else {
                Err(format!("unexpected error in obtaining file info: {:?}", size.status()))
            }
        }
    }
}
