pub mod fonts;
pub mod layout;

////////////////////////////////////////////////////////////////////////////////
// Bit depth
////////////////////////////////////////////////////////////////////////////////

#[cfg(any(
    all(feature = "bpp16", feature = "bpp32"),
    all(feature = "bpp16", feature = "bpp32rgba"),
    all(feature = "bpp32", feature = "bpp32rgba"),
))]
compile_error!("only one of bpp16, bpp32, bpp32rgba may be selected");

#[cfg(not(any(feature = "bpp16", feature = "bpp32", feature = "bpp32rgba")))]
compile_error!("one of bpp16, bpp32, or bpp32rgba must be selected");

#[cfg(feature = "bpp16")]
pub type Pixel = u16;
#[cfg(any(feature = "bpp32", feature = "bpp32rgba"))]
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

/// RGBA little-endian: bytes in memory are [R, G, B, 0xFF], matching canvas ImageData.
#[cfg(feature = "bpp32rgba")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        ($r as u32) | (($g as u32) << 8) | (($b as u32) << 16) | 0xFF000000u32
    };
}

////////////////////////////////////////////////////////////////////////////////
// Input
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Press   { x: u32, y: u32 },
    Release { x: u32, y: u32 },
    Move    { x: u32, y: u32 },
}

////////////////////////////////////////////////////////////////////////////////
// Renderers
////////////////////////////////////////////////////////////////////////////////

pub trait Renderer {
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

    /// Locks the pixel buffer, calls `draw_fn(pixels, stride)`, then presents.
    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize));

    /// Drains pending input events into an internal buffer and returns them.
    /// Returns an empty slice on backends that have no input device.
    fn poll_events(&mut self) -> &[InputEvent] {
        &[]
    }

    /// Returns `true` when a quit event is pending. Defaults to `false`.
    fn poll_quit(&mut self) -> bool {
        false
    }
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
