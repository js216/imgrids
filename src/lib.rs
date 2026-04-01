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
// Key queue (SDL backend → application)
////////////////////////////////////////////////////////////////////////////////

use std::sync::Mutex;
use std::collections::VecDeque;

static KEY_QUEUE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

#[cfg(feature = "sdl")]
pub(crate) fn push_key(k: &str) {
    if let Ok(mut q) = KEY_QUEUE.lock() {
        q.push_back(k.to_owned());
    }
}

pub fn poll_key() -> Option<String> {
    KEY_QUEUE.lock().ok()?.pop_front()
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
pub mod prebaked;

////////////////////////////////////////////////////////////////////////////////
// Backends
////////////////////////////////////////////////////////////////////////////////

/// Alpha-blend an icon into a pixel buffer.
/// `alpha` is `iw * ih` bytes (row-major, 0=bg, 255=fg).
/// The icon is placed at `(x, y)` in the buffer with stride `stride`.
/// Pre-rendered icon alpha mask for `blit_alpha`.
pub struct Icon {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub alpha: &'static [u8],
}

/// Alpha-blend an icon into a pixel buffer.
pub fn blit_alpha_buf(buf: &mut [Pixel], stride: usize, icon: &Icon, fg: Pixel, bg: Pixel) {
    let (fr, fg_, fb) = pixel_to_rgb(fg);
    let (br, bg_, bb) = pixel_to_rgb(bg);
    let alpha_len = icon.alpha.len();
    for row in 0..icon.h {
        let py = icon.y + row;
        if py >= buf.len() / stride { break; }
        let base = py * stride + icon.x;
        for col in 0..icon.w {
            let ai = row * icon.w + col;
            if ai >= alpha_len { break; }
            let a = icon.alpha[ai] as u32;
            let r = (fr * a + br * (255 - a)) / 255;
            let g = (fg_ * a + bg_ * (255 - a)) / 255;
            let b = (fb * a + bb * (255 - a)) / 255;
            buf[base + col] = rgb_to_pixel(r, g, b);
        }
    }
}

#[cfg(feature = "bpp16")]
fn pixel_to_rgb(p: Pixel) -> (u32, u32, u32) {
    (((p as u32 >> 11) & 0x1F) << 3, ((p as u32 >> 5) & 0x3F) << 2, (p as u32 & 0x1F) << 3)
}
#[cfg(feature = "bpp32")]
fn pixel_to_rgb(p: Pixel) -> (u32, u32, u32) {
    ((p as u32 >> 16) & 0xFF, (p as u32 >> 8) & 0xFF, p as u32 & 0xFF)
}
#[cfg(feature = "bpp32rgba")]
fn pixel_to_rgb(p: Pixel) -> (u32, u32, u32) {
    (p as u32 & 0xFF, (p as u32 >> 8) & 0xFF, (p as u32 >> 16) & 0xFF)
}

#[cfg(feature = "bpp16")]
fn rgb_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as Pixel
}
#[cfg(feature = "bpp32")]
fn rgb_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    ((r << 16) | (g << 8) | b) as Pixel
}
#[cfg(feature = "bpp32rgba")]
fn rgb_to_pixel(r: u32, g: u32, b: u32) -> Pixel {
    (r | (g << 8) | (b << 16) | 0xFF000000) as Pixel
}

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

    /// Blit an alpha-mask icon.
    fn blit_alpha(&mut self, icon: &Icon, fg: Pixel, bg: Pixel);

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
    fn blit_alpha(&mut self, icon: &Icon, fg: Pixel, bg: Pixel) {
        (**self).blit_alpha(icon, fg, bg)
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
