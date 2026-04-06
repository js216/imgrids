pub mod fonts;

////////////////////////////////////////////////////////////////////////////////
// Pixel format
////////////////////////////////////////////////////////////////////////////////

pub trait PixelFormat: Copy + Clone + Default + Send + Sync + 'static {
    fn from_rgb(r: u8, g: u8, b: u8) -> Self;
    fn to_rgb(self) -> (u8, u8, u8);
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgb565(pub u16);

impl Rgb565 {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgb565(((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3))
    }
    #[inline]
    pub const fn to_rgb(self) -> (u8, u8, u8) {
        ((((self.0 >> 11) & 0x1F) << 3) as u8,
         (((self.0 >> 5) & 0x3F) << 2) as u8,
         ((self.0 & 0x1F) << 3) as u8)
    }
}
impl PixelFormat for Rgb565 {
    #[inline] fn from_rgb(r: u8, g: u8, b: u8) -> Self { Self::from_rgb(r, g, b) }
    #[inline] fn to_rgb(self) -> (u8, u8, u8) { self.to_rgb() }
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgb888(pub u32);

impl Rgb888 {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgb888((r as u32) << 16 | (g as u32) << 8 | b as u32)
    }
    #[inline]
    pub const fn to_rgb(self) -> (u8, u8, u8) {
        (((self.0 >> 16) & 0xFF) as u8,
         ((self.0 >> 8) & 0xFF) as u8,
         (self.0 & 0xFF) as u8)
    }
}
impl PixelFormat for Rgb888 {
    #[inline] fn from_rgb(r: u8, g: u8, b: u8) -> Self { Self::from_rgb(r, g, b) }
    #[inline] fn to_rgb(self) -> (u8, u8, u8) { self.to_rgb() }
}

/// RGBA little-endian: bytes in memory are [R, G, B, 0xFF], matching canvas ImageData.
#[derive(Copy, Clone, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Rgba8888(pub u32);

impl Rgba8888 {
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Rgba8888(r as u32 | (g as u32) << 8 | (b as u32) << 16 | 0xFF000000)
    }
    #[inline]
    pub const fn to_rgb(self) -> (u8, u8, u8) {
        ((self.0 & 0xFF) as u8,
         ((self.0 >> 8) & 0xFF) as u8,
         ((self.0 >> 16) & 0xFF) as u8)
    }
}
impl PixelFormat for Rgba8888 {
    #[inline] fn from_rgb(r: u8, g: u8, b: u8) -> Self { Self::from_rgb(r, g, b) }
    #[inline] fn to_rgb(self) -> (u8, u8, u8) { self.to_rgb() }
}

/// Convenience macro — requires a `Pixel` type alias implementing `PixelFormat` in scope.
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        <Pixel as $crate::PixelFormat>::from_rgb($r as u8, $g as u8, $b as u8)
    };
    ($t:expr) => {{
        let (r, g, b) = $t;
        <Pixel as $crate::PixelFormat>::from_rgb(r as u8, g as u8, b as u8)
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

pub fn push_key(k: &str) {
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

pub trait Renderer<P: PixelFormat> {
    /// Draw text into a raw pixel buffer. Returns the x coordinate just
    /// past the last drawn pixel (i.e. x + rendered width).
    fn blit(&self, fb: &mut [P], stride: usize, x: usize, y: usize, text: &str) -> usize;

    /// Draw a single character. Returns x past the drawn glyph.
    fn blit_char(&self, fb: &mut [P], stride: usize, x: usize, y: usize, ch: char) -> usize;

    fn cell_height(&self) -> usize;
    fn char_width(&self, c: char) -> usize;
    fn text_width(&self, text: &str) -> usize {
        text.chars().map(|c| self.char_width(c)).sum()
    }
}

pub mod raster;
pub mod prebaked;
pub mod ttf;

////////////////////////////////////////////////////////////////////////////////
// Backends
////////////////////////////////////////////////////////////////////////////////

/// Pre-rendered icon alpha mask for `blit_alpha`.
pub struct Icon {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub alpha: &'static [u8],
}

/// Alpha-blend an icon into a pixel buffer.
pub fn blit_alpha_buf<P: PixelFormat>(buf: &mut [P], stride: usize, icon: &Icon, fg: P, bg: P) {
    let (fr, fg_, fb) = fg.to_rgb();
    let (br, bg_, bb) = bg.to_rgb();
    let (fr, fg_, fb) = (fr as u32, fg_ as u32, fb as u32);
    let (br, bg_, bb) = (br as u32, bg_ as u32, bb as u32);
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
            buf[base + col] = P::from_rgb(r as u8, g as u8, b as u8);
        }
    }
}

pub trait Backend<P: PixelFormat> {
    fn clear(&mut self, color: P);
    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: P,
    );

    fn blit(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str) -> usize;

    /// Blit a single character without String allocation.
    fn blit_char(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, ch: char) -> usize;

    /// Blit an alpha-mask icon.
    fn blit_alpha(&mut self, icon: &Icon, fg: P, bg: P);

    /// Blit text clipped to a maximum x coordinate.
    fn blit_clipped(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str, max_x: usize) -> usize {
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
    fn flush(&mut self) {}
}

impl<P: PixelFormat> Backend<P> for Box<dyn Backend<P>> {
    fn clear(&mut self, color: P) {
        (**self).clear(color)
    }
    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: P,
    ) {
        (**self).fill_rect(x, y, w, h, color)
    }
    fn blit(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str) -> usize {
        (**self).blit(atlas, x, y, text)
    }
    fn blit_char(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, ch: char) -> usize {
        (**self).blit_char(atlas, x, y, ch)
    }
    fn blit_clipped(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str, max_x: usize) -> usize {
        (**self).blit_clipped(atlas, x, y, text, max_x)
    }
    fn poll_events(&mut self) -> &[InputEvent] {
        (**self).poll_events()
    }
    fn flush(&mut self) {
        (**self).flush()
    }
    fn blit_alpha(&mut self, icon: &Icon, fg: P, bg: P) {
        (**self).blit_alpha(icon, fg, bg)
    }
}

pub fn run<P: PixelFormat>(
    mut backend: Box<dyn Backend<P>>,
    mut tick_fn: impl FnMut(&mut dyn Backend<P>),
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

#[cfg(target_os = "emscripten")]
extern "C" { fn emscripten_sleep(ms: u32); }

#[cfg(target_os = "emscripten")]
pub fn sleep(ms: u32) {
    unsafe { emscripten_sleep(ms) };
}
