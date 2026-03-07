// Monospace font renderer — loads a .font file produced by font_to_bin.py,
// pads all glyphs to the widest advance width, composites fg/bg at load time.
// Mirrors mono.c.

use crate::{Pixel, Renderer};
use std::fs;
use std::io;

pub struct MonoAtlas {
    cell_w: usize,
    cell_h: usize,
    /// Flat: [128][cell_w * cell_h], already composited
    glyphs: Vec<Pixel>,
}

impl MonoAtlas {
    pub fn load(path: &str, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        let data = fs::read(path)?;
        let mut cur = 0usize;

        // Header: u32 cell_height
        if data.len() < 4 {
            return Err(io_err("mono: truncated header"));
        }
        let cell_h = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        cur += 4;

        // Advance widths: 128 × u16
        if data.len() < cur + 128 * 2 {
            return Err(io_err("mono: truncated widths"));
        }
        let mut aw = [0u16; 128];
        for i in 0..128 {
            aw[i] = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap());
            cur += 2;
        }

        let cell_w = aw
            .iter()
            .copied()
            .map(|v| v as usize)
            .max()
            .unwrap_or(1)
            .max(1);
        let cells = cell_w * cell_h;

        let fg_r = ((fg >> 16) & 0xFF) as u32;
        let fg_g = ((fg >> 8) & 0xFF) as u32;
        let fg_b = (fg & 0xFF) as u32;
        let bg_r = ((bg >> 16) & 0xFF) as u32;
        let bg_g = ((bg >> 8) & 0xFF) as u32;
        let bg_b = (bg & 0xFF) as u32;

        let mut glyphs = vec![bg; 128 * cells];

        for code in 0..128usize {
            let gw = aw[code] as usize;
            let dst = &mut glyphs[code * cells..(code + 1) * cells];

            if gw > 0 {
                let mask_len = gw * cell_h;
                if data.len() < cur + mask_len {
                    return Err(io_err("mono: truncated glyph"));
                }
                let mask = &data[cur..cur + mask_len];
                cur += mask_len;

                for row in 0..cell_h {
                    for col in 0..cell_w {
                        let alpha = if col < gw {
                            mask[row * gw + col] as u32
                        } else {
                            0
                        };
                        let inv = 255 - alpha;
                        let r = ((fg_r * alpha + bg_r * inv) / 255) as u8;
                        let g = ((fg_g * alpha + bg_g * inv) / 255) as u8;
                        let b = ((fg_b * alpha + bg_b * inv) / 255) as u8;
                        dst[row * cell_w + col] =
                            0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    }
                }
            }
            // else: columns stay as bg (already initialised)
        }

        Ok(MonoAtlas {
            cell_w,
            cell_h,
            glyphs,
        })
    }

    #[inline]
    fn glyph(&self, code: usize) -> &[Pixel] {
        let n = self.cell_w * self.cell_h;
        let i = code & 0x7F;
        &self.glyphs[i * n..i * n + n]
    }
}

impl Renderer for MonoAtlas {
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) {
        let (cw, ch) = (self.cell_w, self.cell_h);
        let mut cx = x;
        for byte in text.bytes() {
            let src = self.glyph(byte as usize);
            for gy in 0..ch {
                let dst_start = (y + gy) * stride + cx;
                fb[dst_start..dst_start + cw].copy_from_slice(&src[gy * cw..(gy + 1) * cw]);
            }
            cx += cw;
        }
    }

    fn cell_height(&self) -> usize {
        self.cell_h
    }
    fn char_width(&self, _: char) -> usize {
        self.cell_w
    }
}

fn io_err(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
