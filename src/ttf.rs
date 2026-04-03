use crate::{PixelFormat, Renderer};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::collections::HashMap;
use std::io;

// --- Atlas (baked pixels for one size+colour combination) --------------------

pub struct TtfAtlas<P: PixelFormat> {
    cell_h: usize,
    /// Fast-path advance width for ASCII 0-127; 0 = no glyph
    ascii_adv: [usize; 128],
    /// Fast-path offset into buf for ASCII 0-127
    ascii_off: [usize; 128],
    /// Extended glyphs (non-ASCII): code point → (offset, advance)
    ext: HashMap<u32, (usize, usize)>,
    /// Flat buffer of all glyph bitmaps concatenated
    buf: Vec<P>,
}

impl<P: PixelFormat> TtfAtlas<P> {
    /// Create an atlas from one or more font files (fallback chain).
    /// Each entry is (path, size). Glyphs are resolved from the first
    /// font that contains them.
    pub fn new(
        fonts: &[(&str, usize)],
        extra: &[u32],
        fg: P,
        bg: P,
    ) -> io::Result<Self> {
        if fonts.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "no fonts"));
        }
        let cell_h = fonts[0].1;
        let missing: Vec<&str> = fonts
            .iter()
            .map(|&(path, _)| path)
            .filter(|path| !std::path::Path::new(path).exists())
            .collect();
        if !missing.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing font files: {}", missing.join(", ")),
            ));
        }
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
        fg: P,
        bg: P,
    ) -> io::Result<Self> {
        Self::new(&[(path, cell_h)], &[], fg, bg)
    }

    fn bake(fonts: &[FontVec], extra: &[u32], cell_h: usize, fg: P, bg: P) -> Self {
        let (fg_rgb, bg_rgb) = channels(fg, bg);

        // Collect all code points to render: ASCII + extra + scan fonts for non-ASCII
        let mut codepoints: Vec<u32> = (0u32..128).collect();
        for &cp in extra {
            if !codepoints.contains(&cp) {
                codepoints.push(cp);
            }
        }
        // Add common ligature codepoints
        for &cp in &[0xFB00u32, 0xFB01, 0xFB02, 0xFB03, 0xFB04] {
            if !codepoints.contains(&cp) { codepoints.push(cp); }
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
            render_cp: u32, // may differ if using a fallback glyph
            font_idx: usize,
            adv: usize,
        }
        // Fallback map: Unicode → ASCII equivalent for missing glyphs
        let fallbacks: &[(u32, u32)] = &[
            (0x2212, 0x002D), // − → -
            (0x00B7, 0x002E), // · → .
        ];
        let mut glyphs: Vec<GlyphInfo> = Vec::new();
        for &cp in &codepoints {
            let candidates = [cp, fallbacks.iter().find(|&&(from, _)| from == cp).map(|&(_, to)| to).unwrap_or(cp)];
            let mut found = false;
            for &try_cp in &candidates {
                if found { break; }
                if let Some(c) = char::from_u32(try_cp) {
                    for (fi, font) in fonts.iter().enumerate() {
                        let scale = compute_scale(font, cell_h);
                        let sf = font.as_scaled(scale);
                        let id = font.glyph_id(c);
                        if id.0 == 0 { continue; }
                        let a = sf.h_advance(id).ceil() as usize;
                        if a > 0 {
                            glyphs.push(GlyphInfo { cp, render_cp: try_cp, font_idx: fi, adv: a });
                            found = true;
                            break;
                        }
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
            let c = char::from_u32(g.render_cp).unwrap();
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
    fn glyph_by_char(&self, c: char) -> Option<(&[P], usize)> {
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

impl<P: PixelFormat> TtfAtlas<P> {
    fn try_ligature(chars: &[char], i: usize, atlas: &Self) -> (char, usize) {
        if chars[i] == 'f' {
            if i + 2 < chars.len() && chars[i+1] == 'f' && chars[i+2] == 'i'
                && atlas.glyph_by_char('\u{FB03}').is_some() { return ('\u{FB03}', 3); }
            if i + 2 < chars.len() && chars[i+1] == 'f' && chars[i+2] == 'l'
                && atlas.glyph_by_char('\u{FB04}').is_some() { return ('\u{FB04}', 3); }
            if i + 1 < chars.len() && chars[i+1] == 'i'
                && atlas.glyph_by_char('\u{FB01}').is_some() { return ('\u{FB01}', 2); }
            if i + 1 < chars.len() && chars[i+1] == 'l'
                && atlas.glyph_by_char('\u{FB02}').is_some() { return ('\u{FB02}', 2); }
            if i + 1 < chars.len() && chars[i+1] == 'f'
                && atlas.glyph_by_char('\u{FB00}').is_some() { return ('\u{FB00}', 2); }
        }
        (chars[i], 1)
    }
}

impl<P: PixelFormat> Renderer<P> for TtfAtlas<P> {
    fn blit(&self, fb: &mut [P], stride: usize, x: usize, y: usize, text: &str) -> usize {
        let ch = self.cell_h;
        let fb_len = fb.len();
        let mut cx = x;
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let (c, skip) = Self::try_ligature(&chars, i, self);
            if skip > 1 { cx = cx.saturating_sub(3); } // tighten ligature kerning
            i += skip;
            if let Some((src, gw)) = self.glyph_by_char(c) {
                for gy in 0..ch {
                    let dst = (y + gy) * stride + cx;
                    if dst + gw <= fb_len {
                        fb[dst..dst + gw]
                            .copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                    }
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
        let chars: Vec<char> = text.chars().collect();
        let mut w = 0;
        let mut i = 0;
        while i < chars.len() {
            let (c, skip) = Self::try_ligature(&chars, i, self);
            w += self.char_width(c);
            // Ligature kerning adjustment must match blit
            if skip > 1 { w = w.saturating_sub(3); }
            i += skip;
        }
        w
    }
}


// --- Helpers -----------------------------------------------------------------

fn compute_scale(font: &FontVec, cell_h: usize) -> PxScale {
    let raw = PxScale::from(cell_h as f32);
    let sf = font.as_scaled(raw);
    let natural_h = sf.ascent() - sf.descent();
    PxScale::from((cell_h as f32 / natural_h) * cell_h as f32)
}

fn channels<P: PixelFormat>(fg: P, bg: P) -> ((u32, u32, u32), (u32, u32, u32)) {
    let (fr, fg_, fb) = fg.to_rgb();
    let (br, bg_, bb) = bg.to_rgb();
    ((fr as u32, fg_ as u32, fb as u32), (br as u32, bg_ as u32, bb as u32))
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

fn blend_into<P: PixelFormat>(
    dst: &mut [P],
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
        *d = P::from_rgb(r as u8, g as u8, b as u8);
    }
}
