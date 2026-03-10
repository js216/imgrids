use crate::Pixel;

/// Blits text into a pixel buffer.
///
/// Implementations pre-composite colours at construction time so `draw`
/// is pure memory copies with no per-pixel arithmetic at render time.
pub trait Renderer: Send + Sync {
    /// Blits `text` into `fb` with the glyph top-left at `(x, y)`.
    /// `stride` is pixels per row, which may exceed the visible width.
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str);

    fn cell_height(&self) -> usize;
    fn char_width(&self, c: char) -> usize;

    /// Total advance width of `text`. Override for proportional fonts.
    fn text_width(&self, text: &str) -> usize {
        text.chars().map(|c| self.char_width(c)).sum()
    }
}

pub mod chars;
pub mod mono;
pub mod shapes;
pub mod tt;

pub use chars::CharsAtlas;
pub use mono::{MonoAtlas, MonoFont};
pub use shapes::ShapesAtlas;
pub use tt::{TtAtlas, TtFont};
