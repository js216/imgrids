// Bitmap font renderer - scales any embedded font to any cell size using
// nearest-neighbour sampling, baking fg/bg at init time.

use crate::{Pixel, Renderer};

/// Describes a compiled-in bitmap font.
///
/// Font files expose a `pub static` of this type; callers pass a reference
/// to `RasterAtlas::new` to select the font at atlas-creation time.
pub struct BitmapFont {
    pub font_w: usize,
    pub font_h: usize,
    pub glyphs: usize,
    /// Row bytes laid out as [glyph][row], flattened: length = glyphs * font_h.
    pub bitmap: &'static [u8],
}

impl BitmapFont {
    #[inline]
    fn row(&self, glyph: usize, row: usize) -> u8 {
        self.bitmap[(glyph % self.glyphs) * self.font_h + row]
    }
}

pub struct RasterAtlas {
    glyph_w: usize,
    glyph_h: usize,
    font: &'static BitmapFont,
    /// Flat: [font.glyphs][glyph_h * glyph_w]
    glyphs: Vec<Pixel>,
}

impl RasterAtlas {
    pub fn new(
        font: &'static BitmapFont,
        glyph_w: usize,
        glyph_h: usize,
        fg: Pixel,
        bg: Pixel,
    ) -> Self {
        let n = glyph_h * glyph_w;
        let mut glyphs = vec![bg; font.glyphs * n];
        for i in 0..font.glyphs {
            rasterise(
                &mut glyphs[i * n..(i + 1) * n],
                glyph_w,
                glyph_h,
                font,
                i,
                fg,
                bg,
            );
        }
        RasterAtlas {
            glyph_w,
            glyph_h,
            font,
            glyphs,
        }
    }

    #[inline]
    fn glyph(&self, code: usize) -> &[Pixel] {
        let n = self.glyph_h * self.glyph_w;
        let i = code % self.font.glyphs;
        &self.glyphs[i * n..i * n + n]
    }
}

impl Renderer for RasterAtlas {
    fn blit(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) {
        let (gw, gh) = (self.glyph_w, self.glyph_h);
        let mut cx = x;
        for byte in text.bytes() {
            let src = self.glyph(byte as usize);
            for gy in 0..gh {
                let dst_start = (y + gy) * stride + cx;
                fb[dst_start..dst_start + gw]
                    .copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
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

fn rasterise(
    dst: &mut [Pixel],
    gw: usize,
    gh: usize,
    font: &BitmapFont,
    ascii: usize,
    fg: Pixel,
    bg: Pixel,
) {
    for dy in 0..gh {
        let sy = (dy * font.font_h) / gh;
        let row = font.row(ascii, sy);
        for dx in 0..gw {
            let sx = (dx * font.font_w) / gw;
            let lit = (row >> sx) & 1 == 1;
            dst[dy * gw + dx] = if lit { fg } else { bg };
        }
    }
}
