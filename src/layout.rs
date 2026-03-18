// Zero-overhead recursive grid layout engine.
//
// ## DSL
//
//   let layout = col(1, vec![
//       row(1, vec![
//           cell(&my_renderer, gen_hello),
//           cell(&my_renderer, gen_world),
//       ]),
//       row(10, vec![
//           cell(&my_renderer, gen_random),
//       ]),
//   ]);
//
//   // Resolve once before the loop — or inside the loop for dynamic layouts.
//   let mut cells = resolve(&layout, x, y, w, h);
//
//   loop {
//       for c in &mut cells { c.draw(fb, stride); }
//   }
//
// ## Design
//
// * `Node` is a plain enum — no allocation during construction beyond the
//   child Vec and the Arc-wrapped gen closure.
// * `resolve` / `resolve_into` walk the tree and write a flat `Vec<Cell>`.
//   No further tree access after that.
// * `Cell::draw` is a dirty-check + one call to `Renderer::draw`; the check
//   is a pointer comparison (same static str address) so it costs ~1 ns.
// * Arc overhead is reference-count bumps at resolve time only — never
//   during the render loop.

use crate::{Pixel, Renderer};
use std::sync::Arc;

// ─── Gen ─────────────────────────────────────────────────────────────────────

/// A cheaply-clonable text generator closure.
pub type Gen = Arc<dyn Fn() -> &'static str + Send + Sync>;

// ─── Node ────────────────────────────────────────────────────────────────────

/// An element of the layout tree.
///
/// Build with the free functions [`cell`], [`row`], [`col`].
pub enum Node<'r> {
    Cell {
        renderer: &'r dyn Renderer,
        gen: Gen,
    },
    /// Splits its box **horizontally** (children side by side).
    Row {
        weight: u32,
        children: Vec<Node<'r>>,
    },
    /// Splits its box **vertically** (children stacked).
    Col {
        weight: u32,
        children: Vec<Node<'r>>,
    },
}

impl<'r> Node<'r> {
    #[inline]
    pub fn weight(&self) -> u32 {
        match self {
            Node::Cell { .. } => 1,
            Node::Row { weight, .. } => *weight,
            Node::Col { weight, .. } => *weight,
        }
    }
}

/// Leaf node — renderer + text generator.
///
/// `f` can be any `Fn() -> &'static str` — a bare fn pointer works directly.
pub fn cell<'r, F>(renderer: &'r dyn Renderer, f: F) -> Node<'r>
where
    F: Fn() -> &'static str + Send + Sync + 'static,
{
    Node::Cell {
        renderer,
        gen: Arc::new(f),
    }
}

/// Splits its bounding box horizontally (children side by side).
pub fn row(weight: u32, children: Vec<Node<'_>>) -> Node<'_> {
    Node::Row { weight, children }
}

/// Splits its bounding box vertically (children stacked).
pub fn col(weight: u32, children: Vec<Node<'_>>) -> Node<'_> {
    Node::Col { weight, children }
}

// ─── Cell (resolved leaf) ────────────────────────────────────────────────────

/// A resolved, positioned leaf.  Stored in a flat `Vec<Cell>` after [`resolve`].
pub struct Cell<'r> {
    pub renderer: &'r dyn Renderer,
    pub gen: Gen,
    pub x: usize,
    pub y: usize,
    last: String,
}

impl<'r> Cell<'r> {
    /// Draw this cell.  Returns `true` if the framebuffer was modified (text
    /// changed since last call).  Use [`force_draw`] after clearing the screen.
    #[inline]
    pub fn draw(&mut self, fb: &mut [Pixel], stride: usize) -> bool {
        let text = (self.gen)();

        // 1. Compare the current content with our stored String
        if text == self.last {
            return false;
        }

        // 2. Fix E0308: Convert the &str to a String to store it
        self.last = text.to_string();

        // 3. Draw to the framebuffer
        self.renderer.draw(fb, stride, self.x, self.y, text);
        true
    }

    /// Unconditional draw — call after blanking the framebuffer.
    #[inline]
    pub fn force_draw(&mut self, fb: &mut [Pixel], stride: usize) {
        let text = (self.gen)();

        // FIX: Convert the &str reference into an owned String
        self.last = text.to_string();

        self.renderer.draw(fb, stride, self.x, self.y, text);
    }
}

// ─── resolve ─────────────────────────────────────────────────────────────────

/// Walk `root` and return a flat `Vec<Cell>` with pre-computed screen positions.
///
/// Text is vertically centred inside each bounding box.
///
/// Call once before the loop for a static layout, or every frame for dynamic
/// layouts.  Use [`resolve_into`] with a pre-allocated Vec to avoid
/// per-frame allocation.
pub fn resolve<'r>(
    root: &'r Node<'r>,
    bx: usize,
    by: usize,
    bw: usize,
    bh: usize,
) -> Vec<Cell<'r>> {
    let mut out = Vec::new();
    resolve_into(root, bx, by, bw, bh, &mut out);
    out
}

/// Like [`resolve`] but appends into a caller-supplied Vec.
///
/// Pre-allocate to avoid per-frame heap allocation:
/// ```rust,ignore
/// let mut cells = Vec::with_capacity(64);
/// loop {
///     cells.clear();
///     resolve_into(&layout, x, y, w, h, &mut cells);
///     for c in &mut cells { c.draw(fb, stride); }
/// }
/// ```
pub fn resolve_into<'r>(
    node: &'r Node<'r>,
    bx: usize,
    by: usize,
    bw: usize,
    bh: usize,
    out: &mut Vec<Cell<'r>>,
) {
    match node {
        Node::Cell { renderer, gen } => {
            let ch = renderer.cell_height();
            let y = by + (bh / 2).saturating_sub(ch / 2);
            out.push(Cell {
                renderer: *renderer,
                gen: Arc::clone(gen),
                x: bx,
                y,
                last: String::new(), // Changed from ""
            });
        }

        Node::Row { children, .. } => {
            split_children(children, bx, by, bw, bh, true, out);
        }

        Node::Col { children, .. } => {
            split_children(children, bx, by, bw, bh, false, out);
        }
    }
}

fn split_children<'r>(
    children: &'r [Node<'r>],
    bx: usize,
    by: usize,
    bw: usize,
    bh: usize,
    horizontal: bool,
    out: &mut Vec<Cell<'r>>,
) {
    if children.is_empty() {
        return;
    }

    let total_weight: u32 = children.iter().map(|c| c.weight()).sum();
    let total_px = if horizontal { bw } else { bh };
    let n = children.len();
    let mut offset = 0usize;

    for (i, child) in children.iter().enumerate() {
        // Last child gets the exact remainder to prevent rounding drift.
        let share = if i == n - 1 {
            total_px - offset
        } else {
            total_px * child.weight() as usize / total_weight as usize
        };

        let (cx, cy, cw, ch) = if horizontal {
            (bx + offset, by, share, bh)
        } else {
            (bx, by + offset, bw, share)
        };

        resolve_into(child, cx, cy, cw, ch, out);
        offset += share;
    }
}
