use crate::Pixel;

/// Owns a pixel surface and can present it.
pub trait Backend {
    fn width(&self) -> usize;
    fn height(&self) -> usize;

    fn clear(&mut self, color: Pixel);
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel);

    fn draw_border(&mut self, x: usize, y: usize, w: usize, h: usize, thickness: usize, color: Pixel) {
        self.fill_rect(x, y, w, thickness, color);
        self.fill_rect(x, y + h - thickness, w, thickness, color);
        self.fill_rect(x, y, thickness, h, color);
        self.fill_rect(x + w - thickness, y, thickness, h, color);
    }

    /// Locks the pixel buffer, calls `draw_fn(pixels, stride)`, then presents.
    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize));

    /// Returns `true` when a quit event is pending. Defaults to `false`.
    fn poll_quit(&mut self) -> bool { false }
}

#[cfg(feature = "fb0")]
pub mod framebuffer;
#[cfg(feature = "fb0")]
pub use framebuffer::init;

#[cfg(feature = "sdl")]
pub mod sdl2;
#[cfg(all(feature = "sdl", not(feature = "web")))]
pub use sdl2::init;

#[cfg(feature = "web")]
pub mod web;
#[cfg(feature = "web")]
pub use web::init;
