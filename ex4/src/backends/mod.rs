use crate::Pixel;

/// Owns a pixel surface and can present it.
///
/// Implement by providing `width`, `height`, `clear`, `fill_rect`, and
/// `render`; the rest have defaults.  Construction is backend-specific and
/// lives in each submodule.
pub trait Backend {
    fn width(&self) -> usize;
    fn height(&self) -> usize;

    fn clear(&mut self, color: Pixel);
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel);

    /// Draws a hollow rectangle; implemented in terms of `fill_rect`.
    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, th: usize, color: Pixel) {
        self.fill_rect(x, y, w, th, color);
        self.fill_rect(x, y + h - th, w, th, color);
        self.fill_rect(x, y, th, h, color);
        self.fill_rect(x + w - th, y, th, h, color);
    }

    /// Locks the pixel buffer, calls `draw_fn(pixels, stride)`, then presents.
    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize));

    /// Returns `true` when a quit event is pending.  Defaults to `false`.
    fn poll_quit(&mut self) -> bool { false }
}

#[cfg(feature = "fb0")]
pub mod framebuffer;
#[cfg(feature = "fb0")]
pub use framebuffer::init;

#[cfg(feature = "sdl")]
pub mod sdl2;
#[cfg(feature = "sdl")]
pub use sdl2::init;
