// Bitmap font renderer — scales the embedded 8×8 public-domain font to any
// cell size using nearest-neighbour sampling, baking fg/bg at init time.
// Mirrors chars.c.

use crate::{Pixel, Renderer};

// The 8×8 font data, embedded at compile time.
// Each entry is 8 bytes; bit N of byte Y is the pixel at column N, row Y.
include!("font8x8.rs");

const FONT_W: usize = 8;
const FONT_H: usize = 8;
const NUM_GLYPHS: usize = 128;

pub struct CharsAtlas {
    glyph_w: usize,
    glyph_h: usize,
    /// Flat: [NUM_GLYPHS][glyph_h * glyph_w]
    glyphs: Vec<Pixel>,
}

impl CharsAtlas {
    pub fn new(glyph_w: usize, glyph_h: usize, fg: Pixel, bg: Pixel) -> Self {
        let n = glyph_h * glyph_w;
        let mut glyphs = vec![bg; NUM_GLYPHS * n];
        for i in 0..NUM_GLYPHS {
            rasterise(&mut glyphs[i * n..(i + 1) * n], glyph_w, glyph_h, i, fg, bg);
        }
        CharsAtlas {
            glyph_w,
            glyph_h,
            glyphs,
        }
    }

    #[inline]
    fn glyph(&self, code: usize) -> &[Pixel] {
        let n = self.glyph_h * self.glyph_w;
        let i = code & 0x7F;
        &self.glyphs[i * n..i * n + n]
    }
}

impl Renderer for CharsAtlas {
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) {
        let (gw, gh) = (self.glyph_w, self.glyph_h);
        let mut cx = x;
        for byte in text.bytes() {
            let src = self.glyph(byte as usize);
            for gy in 0..gh {
                let dst_start = (y + gy) * stride + cx;
                fb[dst_start..dst_start + gw].copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
            }
            cx += gw;
        }
    }

    fn cell_height(&self) -> usize {
        self.glyph_h
    }
    fn char_width(&self, _: char) -> usize {
        self.glyph_w
    }
}

fn rasterise(dst: &mut [Pixel], gw: usize, gh: usize, ascii: usize, fg: Pixel, bg: Pixel) {
    let rows = &FONT8X8[ascii & 0x7F];
    for dy in 0..gh {
        let sy = (dy * FONT_H) / gh;
        for dx in 0..gw {
            let sx = (dx * FONT_W) / gw;
            let lit = (rows[sy] >> sx) & 1 == 1;
            dst[dy * gw + dx] = if lit { fg } else { bg };
        }
    }
}
