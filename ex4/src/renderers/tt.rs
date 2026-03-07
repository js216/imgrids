// Proportional font renderer — loads a .font file from font_to_bin.py,
// composites fg/bg at load time.  Mirrors tt.c.

use crate::{Pixel, Renderer};
use std::fs;
use std::io;

pub struct TtAtlas {
    cell_h: usize,
    /// Advance width per ASCII slot; 0 = no glyph
    adv: [usize; 128],
    /// Flat buffer backing all per-glyph bitmaps (variable width × cell_h)
    buf: Vec<Pixel>,
    /// Byte-offset into buf for each glyph; None if adv == 0
    offsets: [Option<usize>; 128],
}

impl TtAtlas {
    pub fn load(path: &str, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        let data = fs::read(path)?;
        let mut cur = 0usize;

        // Header: u32 cell_height
        if data.len() < 4 {
            return Err(io_err("tt: truncated header"));
        }
        let cell_h = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        cur += 4;

        // Advance widths: 128 × u16
        if data.len() < cur + 128 * 2 {
            return Err(io_err("tt: truncated widths"));
        }
        let mut aw = [0u16; 128];
        for i in 0..128 {
            aw[i] = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap());
            cur += 2;
        }

        let total: usize = aw.iter().map(|&w| w as usize * cell_h).sum();

        let fg_r = ((fg >> 16) & 0xFF) as u32;
        let fg_g = ((fg >> 8) & 0xFF) as u32;
        let fg_b = (fg & 0xFF) as u32;
        let bg_r = ((bg >> 16) & 0xFF) as u32;
        let bg_g = ((bg >> 8) & 0xFF) as u32;
        let bg_b = (bg & 0xFF) as u32;

        let mut buf = vec![0u32; total];
        let mut offsets = [None; 128];
        let mut adv = [0usize; 128];
        let mut ptr = 0usize;

        for code in 0..128usize {
            let gw = aw[code] as usize;
            adv[code] = gw;
            if gw == 0 {
                continue;
            }

            offsets[code] = Some(ptr);
            let n = gw * cell_h;

            if data.len() < cur + n {
                return Err(io_err("tt: truncated glyph"));
            }
            let mask = &data[cur..cur + n];
            cur += n;

            for i in 0..n {
                let alpha = mask[i] as u32;
                let inv = 255 - alpha;
                let r = ((fg_r * alpha + bg_r * inv) / 255) as u8;
                let g = ((fg_g * alpha + bg_g * inv) / 255) as u8;
                let b = ((fg_b * alpha + bg_b * inv) / 255) as u8;
                buf[ptr + i] = 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            }
            ptr += n;
        }

        Ok(TtAtlas {
            cell_h,
            adv,
            buf,
            offsets,
        })
    }

    #[inline]
    fn glyph(&self, code: usize) -> Option<(&[Pixel], usize)> {
        let code = code & 0x7F;
        let gw = self.adv[code];
        if gw == 0 {
            return None;
        }
        let off = self.offsets[code]?;
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
            // if glyph missing, advance by 0 (consistent with adv[code]==0)
        }
    }

    fn cell_height(&self) -> usize {
        self.cell_h
    }

    fn char_width(&self, c: char) -> usize {
        if c.is_ascii() {
            self.adv[c as usize]
        } else {
            0
        }
    }

    fn text_width(&self, text: &str) -> usize {
        text.bytes().map(|b| self.adv[b as usize & 0x7F]).sum()
    }
}

fn io_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
