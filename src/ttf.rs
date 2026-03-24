use crate::{Backend, Pixel, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::io;

// --- Atlas (baked pixels for one size+colour combination) --------------------

pub struct TtfAtlas {
    cell_h: usize,
    /// Per-glyph advance width in pixels; 0 = no glyph
    adv: [usize; 128],
    /// Flat buffer of all glyph bitmaps concatenated (variable width x cell_h)
    buf: Vec<Pixel>,
    /// Index into buf for each glyph
    offsets: [usize; 128],
}

impl TtfAtlas {
    pub fn new(
        path: &str,
        cell_h: usize,
        fg: Pixel,
        bg: Pixel,
    ) -> io::Result<Self> {
        let data = std::fs::read(path)
            .map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
        let font = FontVec::try_from_vec(data).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, e.to_string())
        })?;
        Ok(Self::bake(&font, cell_h, fg, bg))
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

        let mut adv = [0usize; 128];
        let mut offsets = [0usize; 128];
        for code in 0u8..128 {
            adv[code as usize] =
                sf.h_advance(font.glyph_id(code as char)).ceil() as usize;
        }

        let total: usize = adv.iter().map(|&w| w * cell_h).sum();
        let mut buf = vec![bg; total];
        let mut ptr = 0usize;

        let (fg_rgb, bg_rgb) = channels(fg, bg);

        for code in 0u8..128 {
            let gw = adv[code as usize];
            offsets[code as usize] = ptr;
            if gw == 0 {
                continue;
            }

            let id = font.glyph_id(code as char);
            let lsb = sf.h_side_bearing(id);
            let glyph = id.with_scale_and_position(
                scale,
                ab_glyph::point(-lsb, baseline),
            );

            let n = gw * cell_h;
            let mut alpha = vec![0u8; n];

            if let Some(outlined) = font.outline_glyph(glyph) {
                rasterise_alpha(&outlined, &mut alpha, gw, cell_h);
            }

            blend_into(&mut buf[ptr..ptr + n], &alpha, fg_rgb, bg_rgb);
            ptr += n;
        }

        TtfAtlas {
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

impl Renderer for TtfAtlas {
    fn draw(&self, backend: &mut dyn Backend, x: usize, y: usize, text: &str) {
        let ch = self.cell_h;
        backend.render(&mut |fb, stride| {
            let mut cx = x;
            for byte in text.bytes() {
                if let Some((src, gw)) = self.glyph(byte as usize) {
                    for gy in 0..ch {
                        let dst = (y + gy) * stride + cx;
                        fb[dst..dst + gw]
                            .copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                    }
                    cx += gw;
                }
            }
        });
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

// --- Helpers -----------------------------------------------------------------

#[cfg(feature = "bpp16")]
fn to_rgb888(p: Pixel) -> (u32, u32, u32) {
    (
        ((p as u32 >> 11) & 0x1F) << 3,
        ((p as u32 >> 5) & 0x3F) << 2,
        ((p as u32) & 0x1F) << 3,
    )
}

#[cfg(feature = "bpp32")]
fn to_rgb888(p: Pixel) -> (u32, u32, u32) {
    (
        (p as u32 >> 16) & 0xFF,
        (p as u32 >> 8) & 0xFF,
        (p as u32) & 0xFF,
    )
}

#[cfg(feature = "bpp32rgba")]
fn to_rgb888(p: Pixel) -> (u32, u32, u32) {
    (p & 0xFF, (p >> 8) & 0xFF, (p >> 16) & 0xFF)
}

#[cfg(feature = "bpp16")]
fn rgb888_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as Pixel
}

#[cfg(feature = "bpp32")]
fn rgb888_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    ((r << 16) | (g << 8) | b) as Pixel
}

#[cfg(feature = "bpp32rgba")]
fn rgb888_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    r | (g << 8) | (b << 16) | 0xFF000000
}

fn channels(fg: Pixel, bg: Pixel) -> ((u32, u32, u32), (u32, u32, u32)) {
    (to_rgb888(fg), to_rgb888(bg))
}

fn rasterise_alpha(
    outlined: &ab_glyph::OutlinedGlyph,
    alpha: &mut [u8],
    cell_w: usize,
    cell_h: usize,
) {
    let bounds = outlined.px_bounds();
    outlined.draw(|x, y, v| {
        let px = bounds.min.x as i32 + x as i32;
        let py = bounds.min.y as i32 + y as i32;
        if px >= 0
            && py >= 0
            && (px as usize) < cell_w
            && (py as usize) < cell_h
        {
            alpha[py as usize * cell_w + px as usize] = (v * 255.0 + 0.5) as u8;
        }
    });
}

fn blend_into(
    dst: &mut [Pixel],
    alpha: &[u8],
    fg: (u32, u32, u32),
    bg: (u32, u32, u32),
) {
    for (d, &a) in dst.iter_mut().zip(alpha.iter()) {
        let a = a as u32;
        let inv = 255 - a;
        let r = (fg.0 * a + bg.0 * inv) / 255;
        let g = (fg.1 * a + bg.1 * inv) / 255;
        let b = (fg.2 * a + bg.2 * inv) / 255;
        *d = rgb888_to_pixel(r, g, b);
    }
}
