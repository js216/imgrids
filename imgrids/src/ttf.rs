use crate::{PixelFormat, Renderer};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;

/// Atlas created by rasterizing TTF font data at runtime using fontdue.
/// Glyphs are baked into a flat pixel buffer at construction time;
/// subsequent blitting is just memory copies (identical hot path to PrebakedAtlas).
pub struct TtfAtlas<P: PixelFormat> {
    cell_h: usize,
    ascii_adv: [usize; 128],
    ascii_off: [usize; 128],
    ext: HashMap<u32, (usize, usize)>,
    buf: Vec<P>,
}

impl<P: PixelFormat> TtfAtlas<P> {
    /// Create an atlas from one or more embedded font files (fallback chain).
    /// Each entry is (font_data, pixel_size). Glyphs are resolved from the
    /// first font that contains them.
    pub fn new(
        fonts: &[(&[u8], usize)],
        extra: &[u32],
        fg: P,
        bg: P,
    ) -> Self {
        assert!(!fonts.is_empty(), "no fonts");
        let cell_h = fonts[0].1;

        let loaded: Vec<Font> = fonts.iter().map(|&(data, _)| {
            Font::from_bytes(data, FontSettings::default())
                .expect("failed to parse font")
        }).collect();

        Self::bake(&loaded, extra, cell_h, fg, bg)
    }

    fn bake(fonts: &[Font], extra: &[u32], cell_h: usize, fg: P, bg: P) -> Self {
        let px = cell_h as f32;
        let lut = build_lut(fg, bg);

        // Collect all code points: ASCII + extra + ligatures + FontAwesome PUA
        let mut codepoints: Vec<u32> = (0u32..128).collect();
        for &cp in extra {
            if !codepoints.contains(&cp) { codepoints.push(cp); }
        }
        for &cp in &[0xFB00u32, 0xFB01, 0xFB02, 0xFB03, 0xFB04] {
            if !codepoints.contains(&cp) { codepoints.push(cp); }
        }
        // Scan FontAwesome PUA range
        for font in fonts {
            for cp in 0xF000u32..0xFA00 {
                if let Some(c) = char::from_u32(cp) {
                    if font.lookup_glyph_index(c) != 0 && !codepoints.contains(&cp) {
                        codepoints.push(cp);
                    }
                }
            }
        }

        // Unicode → ASCII fallbacks for missing glyphs
        let fallbacks: &[(u32, u32)] = &[
            (0x2212, 0x002D), // − → -
            (0x00B7, 0x002E), // · → .
        ];

        struct GlyphInfo {
            cp: u32,
            render_cp: u32,
            font_idx: usize,
            adv: usize,
        }

        let mut glyphs: Vec<GlyphInfo> = Vec::new();
        for &cp in &codepoints {
            let candidates = [
                cp,
                fallbacks.iter().find(|&&(from, _)| from == cp)
                    .map(|&(_, to)| to).unwrap_or(cp),
            ];
            let mut found = false;
            for &try_cp in &candidates {
                if found { break; }
                if let Some(c) = char::from_u32(try_cp) {
                    for (fi, font) in fonts.iter().enumerate() {
                        if font.lookup_glyph_index(c) == 0 { continue; }
                        let metrics = font.metrics(c, px);
                        let a = metrics.advance_width.ceil() as usize;
                        if a > 0 {
                            glyphs.push(GlyphInfo { cp, render_cp: try_cp, font_idx: fi, adv: a });
                            found = true;
                            break;
                        }
                    }
                }
            }
        }

        // Render all glyphs into a flat pixel buffer
        let total: usize = glyphs.iter().map(|g| g.adv * cell_h).sum();
        let mut buf = vec![P::default(); total];
        let mut ascii_adv = [0usize; 128];
        let mut ascii_off = [0usize; 128];
        let mut ext: HashMap<u32, (usize, usize)> = HashMap::new();
        let mut ptr = 0usize;

        for g in &glyphs {
            let font = &fonts[g.font_idx];
            let c = char::from_u32(g.render_cp).unwrap();
            let gw = g.adv;

            if g.cp < 128 {
                ascii_adv[g.cp as usize] = gw;
                ascii_off[g.cp as usize] = ptr;
            } else {
                ext.insert(g.cp, (ptr, gw));
            }

            let (metrics, alpha_bitmap) = font.rasterize(c, px);

            // Blit the rasterized alpha into the cell, positioned correctly
            let n = gw * cell_h;
            let dst = &mut buf[ptr..ptr + n];
            // Fill with background first
            for p in dst.iter_mut() { *p = lut[0]; }

            // The fontdue bitmap may not fill the entire cell.
            // metrics.xmin/ymin give the offset from the origin.
            let x_off = metrics.xmin.max(0) as usize;
            let y_off = compute_y_offset(font, px, &metrics, cell_h);

            for by in 0..metrics.height {
                let dy = y_off + by;
                if dy >= cell_h { break; }
                for bx in 0..metrics.width {
                    let dx = x_off + bx;
                    if dx >= gw { break; }
                    let a = alpha_bitmap[by * metrics.width + bx];
                    dst[dy * gw + dx] = lut[a as usize];
                }
            }

            ptr += n;
        }

        TtfAtlas { cell_h, ascii_adv, ascii_off, ext, buf }
    }

    #[inline]
    fn glyph_pixels(&self, c: char) -> Option<(&[P], usize)> {
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

    fn try_ligature(chars: &[char], i: usize, atlas: &Self) -> (char, usize) {
        if chars[i] == 'f' {
            if i + 2 < chars.len() && chars[i+1] == 'f' && chars[i+2] == 'i'
                && atlas.glyph_pixels('\u{FB03}').is_some() { return ('\u{FB03}', 3); }
            if i + 2 < chars.len() && chars[i+1] == 'f' && chars[i+2] == 'l'
                && atlas.glyph_pixels('\u{FB04}').is_some() { return ('\u{FB04}', 3); }
            if i + 1 < chars.len() && chars[i+1] == 'i'
                && atlas.glyph_pixels('\u{FB01}').is_some() { return ('\u{FB01}', 2); }
            if i + 1 < chars.len() && chars[i+1] == 'l'
                && atlas.glyph_pixels('\u{FB02}').is_some() { return ('\u{FB02}', 2); }
            if i + 1 < chars.len() && chars[i+1] == 'f'
                && atlas.glyph_pixels('\u{FB00}').is_some() { return ('\u{FB00}', 2); }
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
            if skip > 1 { cx = cx.saturating_sub(3); }
            i += skip;
            if let Some((src, gw)) = self.glyph_pixels(c) {
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

    fn blit_char(&self, fb: &mut [P], stride: usize, x: usize, y: usize, ch: char) -> usize {
        let ch_h = self.cell_h;
        let fb_len = fb.len();
        if let Some((src, gw)) = self.glyph_pixels(ch) {
            for gy in 0..ch_h {
                let dst = (y + gy) * stride + x;
                if dst + gw <= fb_len {
                    fb[dst..dst + gw]
                        .copy_from_slice(&src[gy * gw..(gy + 1) * gw]);
                }
            }
            x + gw
        } else {
            x
        }
    }

    fn cell_height(&self) -> usize { self.cell_h }

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
            if skip > 1 { w = w.saturating_sub(3); }
            i += skip;
        }
        w
    }
}

// --- Helpers -----------------------------------------------------------------

fn build_lut<P: PixelFormat>(fg: P, bg: P) -> [P; 256] {
    let (fr, fg_, fb) = fg.to_rgb();
    let (br, bg_, bb) = bg.to_rgb();
    let (fr, fg_, fb) = (fr as u32, fg_ as u32, fb as u32);
    let (br, bg_, bb) = (br as u32, bg_ as u32, bb as u32);
    let mut lut = [P::default(); 256];
    for a in 0u32..256 {
        let inv = 255 - a;
        lut[a as usize] = P::from_rgb(
            ((fr * a + br * inv) / 255) as u8,
            ((fg_ * a + bg_ * inv) / 255) as u8,
            ((fb * a + bb * inv) / 255) as u8,
        );
    }
    lut
}

/// Compute y offset for a glyph within the cell, aligning to a consistent baseline.
fn compute_y_offset(font: &Font, px: f32, metrics: &fontdue::Metrics, _cell_h: usize) -> usize {
    let line = font.horizontal_line_metrics(px);
    if let Some(lm) = line {
        // ascent is distance from baseline to top of cell
        let baseline_y = lm.ascent.round() as i32;
        // ymin is distance from baseline to top of glyph (positive = above baseline)
        let top = baseline_y - metrics.height as i32 - metrics.ymin;
        top.max(0) as usize
    } else {
        // Fallback: place at top
        0
    }
}
