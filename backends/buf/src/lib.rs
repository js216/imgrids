use imgrids::{blit_alpha_buf, Backend, Icon, InputEvent, PixelFormat, Renderer};

pub struct BufBackend<P: PixelFormat> {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<P>,
    pub blit_count: usize,
    pub fill_rect_count: usize,
}

impl<P: PixelFormat> BufBackend<P> {
    pub fn new(w: usize, h: usize) -> Self {
        BufBackend {
            width: w,
            height: h,
            pixels: vec![P::default(); w * h],
            blit_count: 0,
            fill_rect_count: 0,
        }
    }

    pub fn reset_counts(&mut self) {
        self.blit_count = 0;
        self.fill_rect_count = 0;
    }

    /// Encode the framebuffer as a binary PPM (P6) image.
    pub fn ppm(&self) -> Vec<u8> {
        let header = format!("P6\n{} {}\n255\n", self.width, self.height);
        let mut out = Vec::with_capacity(header.len() + self.width * self.height * 3);
        out.extend_from_slice(header.as_bytes());
        for &px in &self.pixels {
            let (r, g, b) = px.to_rgb();
            out.push(r);
            out.push(g);
            out.push(b);
        }
        out
    }
}

impl<P: PixelFormat> Backend<P> for BufBackend<P> {
    fn clear(&mut self, color: P) {
        self.pixels.fill(color);
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: P) {
        self.fill_rect_count += 1;
        for row in y..y + h {
            if row >= self.height {
                break;
            }
            let end_col = (x + w).min(self.width);
            let start = row * self.width + x;
            let end = row * self.width + end_col;
            self.pixels[start..end].fill(color);
        }
    }

    fn blit(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str) -> usize {
        self.blit_count += 1;
        atlas.blit(&mut self.pixels, self.width, x, y, text)
    }

    fn blit_alpha(&mut self, icon: &Icon, fg: P, bg: P) {
        blit_alpha_buf(&mut self.pixels, self.width, icon, fg, bg);
    }

    fn poll_events(&mut self) -> &[InputEvent] {
        &[]
    }
}
