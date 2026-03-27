use crate::{Pixel, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::collections::HashMap;
use std::io;

// --- Atlas (baked pixels for one size+colour combination) --------------------

pub struct TtfAtlas {
    cell_h: usize,
    /// Fast-path advance width for ASCII 0-127; 0 = no glyph
    ascii_adv: [usize; 128],
    /// Fast-path offset into buf for ASCII 0-127
    ascii_off: [usize; 128],
    /// Extended glyphs (non-ASCII): code point → (offset, advance)
    ext: HashMap<u32, (usize, usize)>,
    /// Flat buffer of all glyph bitmaps concatenated
    buf: Vec<Pixel>,
}

impl TtfAtlas {
    /// Create an atlas from one or more font files (fallback chain).
    /// Each entry is (path, size). Glyphs are resolved from the first
    /// font that contains them.
    pub fn new(
        fonts: &[(&str, usize)],
        extra: &[u32],
        fg: Pixel,
        bg: Pixel,
    ) -> io::Result<Self> {
        if fonts.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no fonts"));
        }
        let cell_h = fonts[0].1;
        let mut loaded = Vec::new();
        for &(path, _) in fonts {
            let data = std::fs::read(path)
                .map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
            let font = FontVec::try_from_vec(data).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("{path}: {e}"))
            })?;
            loaded.push(font);
        }
        Ok(Self::bake(&loaded, extra, cell_h, fg, bg))
    }

    /// Legacy single-font constructor.
    pub fn new_single(
        path: &str,
        cell_h: usize,
        fg: Pixel,
        bg: Pixel,
    ) -> io::Result<Self> {
        Self::new(&[(path, cell_h)], &[], fg, bg)
    }

    fn bake(fonts: &[FontVec], extra: &[u32], cell_h: usize, fg: Pixel, bg: Pixel) -> Self {
        let (fg_rgb, bg_rgb) = channels(fg, bg);

        // Collect all code points to render: ASCII + extra + scan fonts for non-ASCII
        let mut codepoints: Vec<u32> = (0u32..128).collect();
        for &cp in extra {
            if !codepoints.contains(&cp) {
                codepoints.push(cp);
            }
        }
        for font in fonts {
            // Scan Private Use Area (FontAwesome range) and other useful ranges
            for cp in 0xF000u32..0xFA00 {
                if let Some(c) = char::from_u32(cp) {
                    let id = font.glyph_id(c);
                    if id.0 != 0 && !codepoints.contains(&cp) {
                        codepoints.push(cp);
                    }
                }
            }
        }

        // Pre-compute advances: for each code point, find the first font with a glyph
        struct GlyphInfo {
            cp: u32,
            font_idx: usize,
            adv: usize,
        }
        let mut glyphs: Vec<GlyphInfo> = Vec::new();
        for &cp in &codepoints {
            if let Some(c) = char::from_u32(cp) {
                for (fi, font) in fonts.iter().enumerate() {
                    let scale = compute_scale(font, cell_h);
                    let sf = font.as_scaled(scale);
                    let id = font.glyph_id(c);
                    // Skip .notdef (id 0) — means the font doesn't have this character
                    if id.0 == 0 { continue; }
                    let a = sf.h_advance(id).ceil() as usize;
                    if a > 0 {
                        glyphs.push(GlyphInfo { cp, font_idx: fi, adv: a });
                        break;
                    }
                }
            }
        }

        // Render all glyphs into a flat buffer
        let total: usize = glyphs.iter().map(|g| g.adv * cell_h).sum();
        let mut buf = vec![bg; total];
        let mut ascii_adv = [0usize; 128];
        let mut ascii_off = [0usize; 128];
        let mut ext: HashMap<u32, (usize, usize)> = HashMap::new();
        let mut ptr = 0usize;

        for g in &glyphs {
            let font = &fonts[g.font_idx];
            let scale = compute_scale(font, cell_h);
            let sf = font.as_scaled(scale);
            let c = char::from_u32(g.cp).unwrap();
            let gw = g.adv;

            if g.cp < 128 {
                ascii_adv[g.cp as usize] = gw;
                ascii_off[g.cp as usize] = ptr;
            } else {
                ext.insert(g.cp, (ptr, gw));
            }

            let id = font.glyph_id(c);
            let lsb = sf.h_side_bearing(id);
            let glyph = id.with_scale_and_position(
                scale,
                ab_glyph::point(-lsb, sf.ascent()),
            );

            let n = gw * cell_h;
            let mut alpha = vec![0u8; n];
            if let Some(outlined) = font.outline_glyph(glyph) {
                rasterise_alpha(&outlined, &mut alpha, gw, cell_h);
            }
            blend_into(&mut buf[ptr..ptr + n], &alpha, fg_rgb, bg_rgb);
            ptr += n;
        }

        TtfAtlas { cell_h, ascii_adv, ascii_off, ext, buf }
    }

    #[inline]
    fn glyph_by_char(&self, c: char) -> Option<(&[Pixel], usize)> {
        let cp = c as u32;
        let (off, gw) = if cp < 128 {
            let gw = self.ascii_adv[cp as usize];
            if gw == 0 { return None; }
            (self.ascii_off[cp as usize], gw)
        } else {
            let &(off, gw) = self.ext.get(&cp)?;
            (off, gw)
        };
        Some((&self.buf[off..off + gw * self.cell_h], gw))
    }
}

impl Renderer for TtfAtlas {
    fn blit(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) -> usize {
        let ch = self.cell_h;
        let mut cx = x;
        for c in text.chars() {
            if let Some((src, gw)) = self.glyph_by_char(c) {
                for gy in 0..ch {
                    let dst = (y + gy) * stride + cx;
                    fb[dst..dst + gw]
                        .copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                }
                cx += gw;
            }
        }
        cx
    }

    fn cell_height(&self) -> usize {
        self.cell_h
    }
    fn char_width(&self, c: char) -> usize {
        let cp = c as u32;
        if cp < 128 {
            self.ascii_adv[cp as usize]
        } else {
            self.ext.get(&cp).map(|&(_, gw)| gw).unwrap_or(0)
        }
    }
    fn text_width(&self, text: &str) -> usize {
        text.chars().map(|c| self.char_width(c)).sum()
    }
}

// --- Helpers -----------------------------------------------------------------

fn compute_scale(font: &FontVec, cell_h: usize) -> PxScale {
    let raw = PxScale::from(cell_h as f32);
    let sf = font.as_scaled(raw);
    let natural_h = sf.ascent() - sf.descent();
    PxScale::from((cell_h as f32 / natural_h) * cell_h as f32)
}

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
