pub mod fonts;
pub mod layout;

////////////////////////////////////////////////////////////////////////////////
// Bit depth
////////////////////////////////////////////////////////////////////////////////

#[cfg(all(feature = "bpp16", feature = "bpp32"))]
compile_error!("bpp16 and bpp32 are mutually exclusive");

#[cfg(not(any(feature = "bpp16", feature = "bpp32")))]
compile_error!("one of bpp16 or bpp32 must be selected");

#[cfg(feature = "bpp16")]
pub type Pixel = u16;
#[cfg(feature = "bpp32")]
pub type Pixel = u32;

#[cfg(feature = "bpp16")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        ((($r as u16 >> 3) << 11) | (($g as u16 >> 2) << 5) | ($b as u16 >> 3))
    };
}

#[cfg(feature = "bpp32")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        (($r as u32) << 16) | (($g as u32) << 8) | ($b as u32)
    };
}

////////////////////////////////////////////////////////////////////////////////
// Renderers
////////////////////////////////////////////////////////////////////////////////

pub trait Renderer: {
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str);
    fn cell_height(&self) -> usize;
    fn char_width(&self, c: char) -> usize;
    fn text_width(&self, text: &str) -> usize {
        text.chars().map(|c| self.char_width(c)).sum()
    }
}

pub mod raster;
pub mod ttf;

////////////////////////////////////////////////////////////////////////////////
// Backends
////////////////////////////////////////////////////////////////////////////////

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
