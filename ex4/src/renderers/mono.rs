use crate::{Pixel, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::io;

pub struct MonoAtlas {
    cell_w: usize,
    cell_h: usize,
    glyphs: Vec<Pixel>,
}

impl MonoAtlas {
    pub fn from_ttf(path: &str, cell_h: usize, fg: Pixel, bg: Pixel) -> io::Result<Self> {
        let data = std::fs::read(path)
            .map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
        let font = FontVec::try_from_vec(data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        // Scale so that ascent - descent == cell_h exactly.
        let scale = {
            let raw = PxScale::from(cell_h as f32);
            let sf  = font.as_scaled(raw);
            let natural_h = sf.ascent() - sf.descent();
            PxScale::from((cell_h as f32 / natural_h) * cell_h as f32)
        };
        let sf       = font.as_scaled(scale);
        let baseline = sf.ascent();

        // Pass 1: find widest advance width → cell_w.
        let cell_w = (0u8..128)
            .map(|c| sf.h_advance(font.glyph_id(c as char)).ceil() as usize)
            .max()
            .unwrap_or(1)
            .max(1);

        // Decompose fg/bg into 8-bit RGB channels for alpha blending.
        // RGB565: RRRRR_GGGGGG_BBBBB → expand to 8-bit with << 3 / << 2.
        let fg_r = (((fg >> 11) & 0x1F) << 3) as u32;
        let fg_g = (((fg >>  5) & 0x3F) << 2) as u32;
        let fg_b = (( fg        & 0x1F) << 3) as u32;
        let bg_r = (((bg >> 11) & 0x1F) << 3) as u32;
        let bg_g = (((bg >>  5) & 0x3F) << 2) as u32;
        let bg_b = (( bg        & 0x1F) << 3) as u32;

        let cells = cell_w * cell_h;
        let mut glyphs = vec![bg; 128 * cells];
        let mut alpha  = vec![0u8; cells];

        for code in 0u8..128 {
            let id    = font.glyph_id(code as char);
            let lsb   = sf.h_side_bearing(id);
            let glyph = id.with_scale_and_position(
                scale,
                ab_glyph::point(-lsb, baseline),
            );

            alpha.fill(0);

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x as i32 + x as i32;
                    let py = bounds.min.y as i32 + y as i32;
                    if px >= 0 && py >= 0
                        && (px as usize) < cell_w
                        && (py as usize) < cell_h
                    {
                        alpha[py as usize * cell_w + px as usize] = (v * 255.0 + 0.5) as u8;
                    }
                });
            }

            let dst = &mut glyphs[code as usize * cells..(code as usize + 1) * cells];
            for (i, &a) in alpha.iter().enumerate() {
                let a   = a as u32;
                let inv = 255 - a;
                let r = ((fg_r * a + bg_r * inv) / 255) as u16;
                let g = ((fg_g * a + bg_g * inv) / 255) as u16;
                let b = ((fg_b * a + bg_b * inv) / 255) as u16;
                dst[i] = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
            }
        }

        Ok(MonoAtlas { cell_w, cell_h, glyphs })
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

    fn cell_height(&self) -> usize { self.cell_h }
    fn char_width(&self, _: char) -> usize { self.cell_w }
}
