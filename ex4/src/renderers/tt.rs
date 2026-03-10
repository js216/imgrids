use crate::{Pixel, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::io;

use super::mono::{blend_into, channels, rasterise_alpha};

// ─── Font (parsed TTF, shareable) ────────────────────────────────────────────

pub struct TtFont {
    font: FontVec,
}

impl TtFont {
    pub fn load(path: &str) -> io::Result<Self> {
        let data =
            std::fs::read(path).map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
        let font = FontVec::try_from_vec(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(TtFont { font })
    }

    pub fn at(&self, cell_h: usize, fg: Pixel, bg: Pixel) -> TtAtlas {
        TtAtlas::bake(&self.font, cell_h, fg, bg)
    }
}

// ─── Atlas (baked pixels for one size+colour combination) ────────────────────

pub struct TtAtlas {
    cell_h: usize,
    /// Per-glyph advance width in pixels; 0 = no glyph
    adv: [usize; 128],
    /// Flat buffer of all glyph bitmaps concatenated (variable width × cell_h)
    buf: Vec<Pixel>,
    /// Index into buf for each glyph
    offsets: [usize; 128],
}

impl TtAtlas {
    /// Convenience: load font and bake in one step.
    pub fn from_ttf(path: &str, cell_h: usize, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        Ok(TtFont::load(path)?.at(cell_h, fg, bg))
    }

    fn bake(font: &FontVec, cell_h: usize, fg: Pixel, bg: Pixel) -> Self {
        let scale = {
            let raw = PxScale::from(cell_h as f32);
            let sf = font.as_scaled(raw);
            let natural_h = sf.ascent() - sf.descent();
            PxScale::from((cell_h as f32 / natural_h) * cell_h as f32)
        };
        let sf = font.as_scaled(scale);
        let baseline = sf.ascent();

        // Measure all advance widths up front so we can allocate buf once.
        let mut adv = [0usize; 128];
        let mut offsets = [0usize; 128];
        for code in 0u8..128 {
            adv[code as usize] = sf.h_advance(font.glyph_id(code as char)).ceil() as usize;
        }

        let total: usize = adv.iter().map(|&w| w * cell_h).sum();
        let mut buf = vec![bg; total];
        let mut ptr = 0usize;

        let (fg_r, fg_g, fg_b, bg_r, bg_g, bg_b) = channels(fg, bg);

        for code in 0u8..128 {
            let gw = adv[code as usize];
            offsets[code as usize] = ptr;
            if gw == 0 {
                continue;
            }

            let id = font.glyph_id(code as char);
            let lsb = sf.h_side_bearing(id);
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(-lsb, baseline));

            let n = gw * cell_h;
            let mut alpha = vec![0u8; n];

            if let Some(outlined) = font.outline_glyph(glyph) {
                rasterise_alpha(&outlined, &mut alpha, gw, cell_h);
            }

            blend_into(
                &mut buf[ptr..ptr + n],
                &alpha,
                fg_r,
                fg_g,
                fg_b,
                bg_r,
                bg_g,
                bg_b,
            );
            ptr += n;
        }

        TtAtlas {
            cell_h,
            adv,
            buf,
            offsets,
        }
    }

    #[inline]
    fn glyph(&self, code: usize) -> Option<(&[Pixel], usize)> {
        let code = code & 0x7F;
        let gw = self.adv[code];
        if gw == 0 {
            return None;
        }
        let off = self.offsets[code];
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
                    let dst = (y + gy) * stride + cx;
                    fb[dst..dst + gw].copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                }
                cx += gw;
            }
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
