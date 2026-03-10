use crate::{Pixel, Renderer};
use std::fs;
use std::io;

pub struct TtAtlas {
    cell_h:  usize,
    adv:     [usize; 128],
    buf:     Vec<Pixel>,
    offsets: [Option<usize>; 128],
}

impl TtAtlas {
    pub fn load(path: &str, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        let data = fs::read(path)?;
        let mut cur = 0usize;

        if data.len() < 4 {
            return Err(io_err("tt: truncated header"));
        }
        let cell_h = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        cur += 4;

        if data.len() < cur + 128 * 2 {
            return Err(io_err("tt: truncated widths"));
        }
        let mut aw = [0u16; 128];
        for i in 0..128 {
            aw[i] = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap());
            cur += 2;
        }

        // Decompose fg/bg RGB565 → 8-bit channels for alpha blending.
        let fg_r = (((fg >> 11) & 0x1F) << 3) as u32;
        let fg_g = (((fg >>  5) & 0x3F) << 2) as u32;
        let fg_b = (( fg        & 0x1F) << 3) as u32;
        let bg_r = (((bg >> 11) & 0x1F) << 3) as u32;
        let bg_g = (((bg >>  5) & 0x3F) << 2) as u32;
        let bg_b = (( bg        & 0x1F) << 3) as u32;

        let total: usize = aw.iter().map(|&w| w as usize * cell_h).sum();
        let mut buf     = vec![0u16; total];
        let mut offsets = [None; 128];
        let mut adv     = [0usize; 128];
        let mut ptr     = 0usize;

        for code in 0..128usize {
            let gw = aw[code] as usize;
            adv[code] = gw;
            if gw == 0 { continue; }

            offsets[code] = Some(ptr);
            let n = gw * cell_h;

            if data.len() < cur + n {
                return Err(io_err("tt: truncated glyph"));
            }
            let mask = &data[cur..cur + n];
            cur += n;

            for i in 0..n {
                let a   = mask[i] as u32;
                let inv = 255 - a;
                let r = ((fg_r * a + bg_r * inv) / 255) as u16;
                let g = ((fg_g * a + bg_g * inv) / 255) as u16;
                let b = ((fg_b * a + bg_b * inv) / 255) as u16;
                buf[ptr + i] = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
            }
            ptr += n;
        }

        Ok(TtAtlas { cell_h, adv, buf, offsets })
    }

    #[inline]
    fn glyph(&self, code: usize) -> Option<(&[Pixel], usize)> {
        let code = code & 0x7F;
        let gw   = self.adv[code];
        if gw == 0 { return None; }
        let off  = self.offsets[code]?;
        Some((&self.buf[off..off + gw * self.cell_h], gw))
    }
}

impl Renderer for TtAtlas {
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) {
        let ch = self.cell_h;
        let mut cx = x;
        for byte in text.bytes() {
            if let Some((src, gw)) = self.glyph(byte as usize) {
                for gy in 0..ch {
                    let dst_start = (y + gy) * stride + cx;
                    fb[dst_start..dst_start + gw].copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                }
                cx += gw;
            }
        }
    }

    fn cell_height(&self) -> usize { self.cell_h }
    fn char_width(&self, c: char) -> usize {
        if c.is_ascii() { self.adv[c as usize] } else { 0 }
    }
    fn text_width(&self, text: &str) -> usize {
        text.bytes().map(|b| self.adv[b as usize & 0x7F]).sum()
    }
}

fn io_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
