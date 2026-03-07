// The one trait that shapes, chars, mono, tt (and any future renderer) must
// implement.  The entire layout engine only knows about this trait.

use crate::Pixel;

/// A glyph atlas that can blit text into a pixel framebuffer.
///
/// All implementations pre-composite colours at construction time so that
/// `draw` is reduced to bulk memory copies — no per-pixel arithmetic at
/// render time.
///
/// # Implementing a new renderer
///
/// 1. Create a struct that holds your atlas data.
/// 2. `impl Renderer for YourAtlas { … }`
/// 3. Pass `Box<dyn Renderer>` or `&dyn Renderer` to `Cell`.
///
/// That's it.  The layout engine is oblivious to the concrete type.
pub trait Renderer: Send + Sync {
    /// Blit `text` into `fb` with the top-left of the first glyph at `(x, y)`.
    ///
    /// * `fb`     — flat pixel slice, row-major
    /// * `stride` — pixels per row (may be wider than the visible area)
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str);

    /// Height of one glyph row in pixels.  Used by the layout engine to
    /// vertically centre text inside its bounding box.
    fn cell_height(&self) -> usize;

    /// Width of a rendered string in pixels.  The default sums `char_width`
    /// over every character; override for proportional fonts.
    fn text_width(&self, text: &str) -> usize {
        text.chars().map(|c| self.char_width(c)).sum()
    }

    /// Advance width of a single character.  Must be consistent with `draw`.
    fn char_width(&self, c: char) -> usize;
}
