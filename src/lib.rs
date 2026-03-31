pub mod fonts;

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
    ($t:expr) => {{
        let (r, g, b) = $t;
        rgb!(r, g, b)
    }};
}

#[cfg(feature = "bpp32")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        (($r as u32) << 16) | (($g as u32) << 8) | ($b as u32)
    };
    ($t:expr) => {{
        let (r, g, b) = $t;
        rgb!(r, g, b)
    }};
}

/// RGBA little-endian: bytes in memory are [R, G, B, 0xFF], matching canvas ImageData.
#[cfg(feature = "bpp32rgba")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        ($r as u32) | (($g as u32) << 8) | (($b as u32) << 16) | 0xFF000000u32
    };
    ($t:expr) => {{
        let (r, g, b) = $t;
        rgb!(r, g, b)
    }};
}

////////////////////////////////////////////////////////////////////////////////
// Input
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy)]
pub enum InputEvent {
    Press { x: usize, y: usize },
    Release { x: usize, y: usize },
    Move { x: usize, y: usize },
    Quit,
}

////////////////////////////////////////////////////////////////////////////////
// Renderers
////////////////////////////////////////////////////////////////////////////////

pub trait Renderer {
    /// Draw text into a raw pixel buffer. Returns the x coordinate just
    /// past the last drawn pixel (i.e. x + rendered width).
    fn blit(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) -> usize;

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
    fn clear(&mut self, color: Pixel);
    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Pixel,
    );

    fn blit(&mut self, atlas: &dyn Renderer, x: usize, y: usize, text: &str) -> usize;

    /// Blit text clipped to a maximum x coordinate.
    fn blit_clipped(&mut self, atlas: &dyn Renderer, x: usize, y: usize, text: &str, max_x: usize) -> usize {
        // Find how many chars fit within max_x
        let mut w = 0;
        let mut end = 0;
        for (i, c) in text.char_indices() {
            let cw = atlas.char_width(c);
            if x + w + cw > max_x { break; }
            w += cw;
            end = i + c.len_utf8();
        }
        self.blit(atlas, x, y, &text[..end])
    }

    /// Drains pending input events into an internal buffer and returns them.
    fn poll_events(&mut self) -> &[InputEvent] {
        &[]
    }

    /// Present the completed frame to the display.
    /// Backends that double-buffer (e.g. SDL2) copy the back-buffer and flip
    /// here; backends that write directly to the display (framebuffer, web)
    /// can leave this as the default no-op.
    fn flush(&mut self) {}
}

impl Backend for Box<dyn Backend> {
    fn clear(&mut self, color: Pixel) {
        (**self).clear(color)
    }
    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Pixel,
    ) {
        (**self).fill_rect(x, y, w, h, color)
    }
    fn blit(&mut self, atlas: &dyn Renderer, x: usize, y: usize, text: &str) -> usize {
        (**self).blit(atlas, x, y, text)
    }
    fn blit_clipped(&mut self, atlas: &dyn Renderer, x: usize, y: usize, text: &str, max_x: usize) -> usize {
        (**self).blit_clipped(atlas, x, y, text, max_x)
    }
    fn poll_events(&mut self) -> &[InputEvent] {
        (**self).poll_events()
    }
    fn flush(&mut self) {
        (**self).flush()
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
#[cfg(target_os = "emscripten")]
pub use web::run;
#[cfg(target_os = "emscripten")]
pub use web::sleep;

#[cfg(not(target_os = "emscripten"))]
pub fn run(
    mut backend: Box<dyn Backend>,
    mut tick_fn: impl FnMut(&mut dyn Backend),
) {
    'main: loop {
        for ev in backend.poll_events() {
            if let InputEvent::Quit = ev {
                break 'main;
            }
        }
        tick_fn(&mut *backend);
        sleep(33);
    }
}

#[cfg(not(target_os = "emscripten"))]
pub fn sleep(ms: u32) {
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
}
