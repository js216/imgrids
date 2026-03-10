use crate::{Pixel, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::io;

// ─── Font (parsed TTF, shareable) ────────────────────────────────────────────

pub struct MonoFont {
    font: FontVec,
}

impl MonoFont {
    pub fn load(path: &str) -> io::Result<Self> {
        let data =
            std::fs::read(path).map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
        let font = FontVec::try_from_vec(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(MonoFont { font })
    }

    pub fn at(&self, cell_h: usize, fg: Pixel, bg: Pixel) -> MonoAtlas {
        MonoAtlas::bake(&self.font, cell_h, fg, bg)
    }
}

// ─── Atlas (baked pixels for one size+colour combination) ────────────────────

pub struct MonoAtlas {
    cell_w: usize,
    cell_h: usize,
    /// Flat: [128][cell_w * cell_h], composited fg/bg in native RGB565
    glyphs: Vec<Pixel>,
}

impl MonoAtlas {
    /// Convenience: load font and bake in one step.
    pub fn from_ttf(path: &str, cell_h: usize, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        Ok(MonoFont::load(path)?.at(cell_h, fg, bg))
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

        // cell_w = widest advance across all 128 glyphs
        let cell_w = (0u8..128)
            .map(|c| sf.h_advance(font.glyph_id(c as char)).ceil() as usize)
            .max()
            .unwrap_or(1)
            .max(1);

        let (fg_r, fg_g, fg_b, bg_r, bg_g, bg_b) = channels(fg, bg);

        let cells = cell_w * cell_h;
        let mut glyphs = vec![bg; 128 * cells];
        let mut alpha = vec![0u8; cells];

        for code in 0u8..128 {
            let id = font.glyph_id(code as char);
            let lsb = sf.h_side_bearing(id);
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(-lsb, baseline));

            alpha.fill(0);
            if let Some(outlined) = font.outline_glyph(glyph) {
                rasterise_alpha(&outlined, &mut alpha, cell_w, cell_h);
            }

            blend_into(
                &mut glyphs[code as usize * cells..(code as usize + 1) * cells],
                &alpha,
                fg_r,
                fg_g,
                fg_b,
                bg_r,
                bg_g,
                bg_b,
            );
        }

        MonoAtlas {
            cell_w,
            cell_h,
            glyphs,
        }
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
                let dst = (y + gy) * stride + cx;
                fb[dst..dst + cw].copy_from_slice(&src[gy * cw..(gy + 1) * cw]);
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

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Expand RGB565 fg/bg into 8-bit channels for alpha blending.
pub(super) fn channels(fg: Pixel, bg: Pixel) -> (u32, u32, u32, u32, u32, u32) {
    let ch = |p: Pixel, shift: u32, mask: u32, expand: u32| -> u32 {
        ((p as u32 >> shift) & mask) << expand
    };
    (
        ch(fg, 11, 0x1F, 3),
        ch(fg, 5, 0x3F, 2),
        ch(fg, 0, 0x1F, 3),
        ch(bg, 11, 0x1F, 3),
        ch(bg, 5, 0x3F, 2),
        ch(bg, 0, 0x1F, 3),
    )
}

/// Rasterise an outlined glyph into an alpha buffer (clamped to cell bounds).
pub(super) fn rasterise_alpha(
    outlined: &ab_glyph::OutlinedGlyph,
    alpha: &mut [u8],
    cell_w: usize,
    cell_h: usize,
) {
    let bounds = outlined.px_bounds();
    outlined.draw(|x, y, v| {
        let px = bounds.min.x as i32 + x as i32;
        let py = bounds.min.y as i32 + y as i32;
        if px >= 0 && py >= 0 && (px as usize) < cell_w && (py as usize) < cell_h {
            alpha[py as usize * cell_w + px as usize] = (v * 255.0 + 0.5) as u8;
        }
    });
}

/// Alpha-blend an 8-bit alpha buffer into a Pixel slice (RGB565 output).
pub(super) fn blend_into(
    dst: &mut [Pixel],
    alpha: &[u8],
    fg_r: u32,
    fg_g: u32,
    fg_b: u32,
    bg_r: u32,
    bg_g: u32,
    bg_b: u32,
) {
    for (d, &a) in dst.iter_mut().zip(alpha.iter()) {
        let a = a as u32;
        let inv = 255 - a;
        let r = ((fg_r * a + bg_r * inv) / 255) as u16;
        let g = ((fg_g * a + bg_g * inv) / 255) as u16;
        let b = ((fg_b * a + bg_b * inv) / 255) as u16;
        *d = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
    }
}
