use crate::{draw, pixel};
use core::fmt::{self, Write};
use nova_core::Framebuffer;
const BG: u32 = 0x10131b;
const FG: u32 = 0xe5e9f0;
pub struct Terminal {
    fb: Framebuffer,
    x: usize,
    y: usize,
    scroll_top: usize,
}
impl Terminal {
    pub fn new(fb: Framebuffer) -> Self {
        Self { fb, x: 48, y: 336, scroll_top: 336 }
    }
    pub fn clear(&mut self) {
        let Some(_draw)=crate::graphics::begin_console() else { return };
        for y in 0..self.fb.height as usize {
            for x in 0..self.fb.width as usize {
                pixel(&self.fb, x, y, BG);
            }
        }
        self.x = 48;
        self.y = 32;
        self.scroll_top = 32;
    }
    fn rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for yy in y..(y + h).min(self.fb.height as usize) {
            for xx in x..(x + w).min(self.fb.width as usize) {
                pixel(&self.fb, xx, yy, color);
            }
        }
    }
    fn scroll(&mut self) {
        let top = self.scroll_top;
        let bottom = self.fb.height as usize;
        for y in top + 12..bottom {
            for x in 0..self.fb.width as usize {
                let offset = (y * self.fb.stride as usize + x) * 4;
                if offset + 4 <= self.fb.size as usize {
                    // SAFETY: both source and destination offsets are checked within the GOP buffer; volatile reads/writes target MMIO.
                    unsafe {
                        let src = (self.fb.base as *mut u8).add(offset) as *mut u32;
                        let dst = (self.fb.base as *mut u8)
                            .add(offset - 12 * self.fb.stride as usize * 4)
                            as *mut u32;
                        dst.write_volatile(src.read_volatile());
                    }
                }
            }
        }
        self.rect(0, bottom - 12, self.fb.width as usize, 12, BG);
        self.y -= 12;
    }
    pub fn backspace(&mut self) {
        let Some(_draw)=crate::graphics::begin_console() else {
            crate::serial_byte(8);
            crate::serial_byte(b' ');
            crate::serial_byte(8);
            return;
        };
        if self.x > 48 {
            self.x -= 9;
            self.rect(self.x, self.y, 9, 9, BG);
            crate::serial_byte(8);
            crate::serial_byte(b' ');
            crate::serial_byte(8);
        }
    }
    pub fn char(&mut self, c: u8) {
        crate::serial_byte(c);
        let Some(_draw)=crate::graphics::begin_console() else { return };
        if c == b'\n' {
            self.x = 48;
            self.y += 12;
        } else if c >= 32 && c < 127 {
            if self.x + 9 >= self.fb.width as usize {
                self.x = 48;
                self.y += 12;
            }
            self.rect(self.x, self.y, 9, 9, BG);
            let b = [c];
            draw(
                &self.fb,
                self.x,
                self.y,
                core::str::from_utf8(&b).unwrap_or("?"),
                FG,
            );
            self.x += 9;
        }
        if self.y + 12 >= self.fb.height as usize {
            self.scroll();
        }
    }
}
impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            self.char(b);
        }
        Ok(())
    }
}
