// The one trait that framebuffer, sdl2 (and any future backend) must
// implement.  The demo examples only know about this trait.
use crate::Pixel;

/// A display backend that owns a pixel surface and can present it.
///
/// All implementations expose the same draw primitives so that demo code
/// is backend-agnostic.  The only backend-specific code is construction.
///
/// # Implementing a new backend
///
/// 1. Create a struct that owns your display surface.
/// 2. `impl Backend for YourBackend { … }`
/// 3. Pass it to your demo as `&mut dyn Backend`.
///
/// That's it.  Demo logic is oblivious to the concrete type.
pub trait Backend {
    /// Width of the display surface in pixels.
    fn width(&self) -> usize;
    /// Height of the display surface in pixels.
    fn height(&self) -> usize;

    /// Fill the entire surface with `color`.
    fn clear(&mut self, color: Pixel);

    /// Fill an axis-aligned rectangle with `color`.
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel);

    /// Draw a hollow border rectangle of `thickness` pixels.
    fn draw_border(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        thickness: usize,
        color: Pixel,
    ) {
        self.fill_rect(x, y, w, thickness, color);
        self.fill_rect(x, y + h - thickness, w, thickness, color);
        self.fill_rect(x, y, thickness, h, color);
        self.fill_rect(x + w - thickness, y, thickness, h, color);
    }

    /// Lock the pixel buffer, invoke `draw_fn`, then present.
    ///
    /// * `draw_fn(pixels, stride)` — flat pixel slice and pixels-per-row
    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize));
}
