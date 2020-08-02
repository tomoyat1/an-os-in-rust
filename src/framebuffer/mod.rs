extern crate uefi;

use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};

use log::info;

const SQUARE_SIZE: usize = 50;

pub mod fonts;
use crate::framebuffer::fonts::{FONT_HEIGHT, FONT_WIDTH};

pub struct Framebuffer<'boot> {
    gop: &'boot mut GraphicsOutput<'boot>,

    pixel_width: usize,
    pixel_height: usize,

    n_col: usize,
    n_row: usize,

    cursor_x: usize,
    cursor_y: usize,

    font: fonts::Terminus16x18Font,
}

impl<'boot> Framebuffer<'boot> {
    pub fn new(system_table: &SystemTable<Boot>) -> Framebuffer {
        let bs = system_table.boot_services();
        let gop = bs
            .locate_protocol::<GraphicsOutput>()
            .expect("Graphics Output Protocol support is required!");
        let gop = gop.expect("warnings occured when opening GOP");
        let gop = unsafe { &mut *gop.get() };
        let (width, height) = gop.current_mode_info().resolution();
        let (nc, nr) = (width / FONT_WIDTH, height / FONT_HEIGHT);
        let fb = Framebuffer {
            gop,
            pixel_width: width,
            pixel_height: height,
            n_col: nc,
            n_row: nr,
            cursor_x: 0,
            cursor_y: 0,
            font: fonts::parse_bdf(),
        };
        return fb;
    }

    pub fn init(&mut self) -> Result<(), ()> {
        let mode_info = self.gop.current_mode_info();
        let (width_px, height_px) = mode_info.resolution();
        match self.gop.blt(BltOp::VideoFill {
            color: BltPixel::new(0x35, 0x33, 0x2b),
            dest: (0, 0),
            dims: (width_px, height_px),
        }) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn write_char_impl(&mut self, c: char) {
        const BUF_SIZE: usize = FONT_HEIGHT * FONT_WIDTH;
        if c == '\n' {
            self.newline();
            return
        }
        let bitmap = self.font.glyphs[c as usize - 32].bitmap;
        let mut buffer = Vec::<BltPixel>::with_capacity(BUF_SIZE);
        for row in bitmap.iter() {
            for b in row {
                buffer.push(if *b == 1 {
                    BltPixel::new(0xee, 0xee, 0xee)
                } else {
                    BltPixel::new(0x35, 0x33, 0x2b)
                });
            }
        }
        self.gop.blt(BltOp::BufferToVideo {
            buffer: &buffer,
            src: BltRegion::SubRectangle {
                coords: (0, 0),
                px_stride: FONT_WIDTH,
            },
            dest: (self.cursor_x * FONT_WIDTH, self.cursor_y * FONT_HEIGHT),
            dims: (FONT_WIDTH, FONT_HEIGHT),
        });
        self.cursor_x += 1;
    }

    pub fn newline(&mut self) {
        if self.cursor_y < self.n_row {
            self.cursor_x = 0;
            self.cursor_y += 1;
        }
        // TODO: implement scrolling
    }
}

impl<'boot> core::fmt::Write for Framebuffer<'boot> {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        for c in s.chars() {
            self.write_char_impl(c);
        };
        Ok(())
    }
}
