// Geometric shapes renderer: square, circle, triangle — cycled by ASCII value.
// Mirrors shapes.c exactly but in safe Rust.

use crate::{Pixel, Renderer};

#[derive(Clone, Copy)]
enum Shape {
    Square,
    Circle,
    Triangle,
}

impl Shape {
    fn from_index(i: usize) -> Self {
        match i % 3 {
            0 => Shape::Square,
            1 => Shape::Circle,
            _ => Shape::Triangle,
        }
    }
}

pub struct ShapesAtlas {
    cell_size: usize,
    /// Flat: [128][cell_size * cell_size]
    glyphs: Vec<Pixel>,
}

impl ShapesAtlas {
    pub fn new(cell_size: usize, fg: Pixel, bg: Pixel) -> Self {
        let n = cell_size * cell_size;
        let mut glyphs = vec![bg; 128 * n];
        for i in 0..128 {
            let dst = &mut glyphs[i * n..(i + 1) * n];
            rasterise(dst, cell_size, Shape::from_index(i), fg, bg);
        }
        ShapesAtlas { cell_size, glyphs }
    }

    #[inline]
    fn glyph(&self, code: usize) -> &[Pixel] {
        let n = self.cell_size * self.cell_size;
        &self.glyphs[(code & 0x7F) * n..(code & 0x7F) * n + n]
    }
}

impl Renderer for ShapesAtlas {
    fn draw(&self, fb: &mut [Pixel], stride: usize, x: usize, y: usize, text: &str) {
        let sz = self.cell_size;
        let mut cx = x;
        for byte in text.bytes() {
            let src = self.glyph(byte as usize);
            for gy in 0..sz {
                let dst_start = (y + gy) * stride + cx;
                fb[dst_start..dst_start + sz].copy_from_slice(&src[gy * sz..(gy + 1) * sz]);
            }
            cx += sz;
        }
    }

    fn cell_height(&self) -> usize {
        self.cell_size
    }
    fn char_width(&self, _: char) -> usize {
        self.cell_size
    }
}

// ─── Rasterisers ─────────────────────────────────────────────────────────────

fn rasterise(dst: &mut [Pixel], sz: usize, shape: Shape, fg: Pixel, bg: Pixel) {
    match shape {
        Shape::Square => rasterise_square(dst, sz, fg, bg),
        Shape::Circle => rasterise_circle(dst, sz, fg, bg),
        Shape::Triangle => rasterise_triangle(dst, sz, fg, bg),
    }
}

fn rasterise_square(dst: &mut [Pixel], sz: usize, fg: Pixel, bg: Pixel) {
    let m = sz / 4;
    for y in 0..sz {
        for x in 0..sz {
            dst[y * sz + x] = if x >= m && x < sz - m && y >= m && y < sz - m {
                fg
            } else {
                bg
            };
        }
    }
}

fn rasterise_circle(dst: &mut [Pixel], sz: usize, fg: Pixel, bg: Pixel) {
    let cx = sz as f32 / 2.0;
    let cy = sz as f32 / 2.0;
    let r = sz as f32 / 2.0 - sz as f32 / 8.0;
    for y in 0..sz {
        let dy = y as f32 - cy;
        for x in 0..sz {
            let dx = x as f32 - cx;
            dst[y * sz + x] = if dx * dx + dy * dy <= r * r { fg } else { bg };
        }
    }
}

fn rasterise_triangle(dst: &mut [Pixel], sz: usize, fg: Pixel, bg: Pixel) {
    let m = sz / 4;
    let (ax, ay) = (sz / 2, m);
    let (bx, by) = (m, sz - m);
    let (cx, cy) = (sz - m, sz - m);

    for y in 0..sz {
        for x in 0..sz {
            let d0 = (bx as i32 - ax as i32) * (y as i32 - ay as i32)
                - (by as i32 - ay as i32) * (x as i32 - ax as i32);
            let d1 = (cx as i32 - bx as i32) * (y as i32 - by as i32)
                - (cy as i32 - by as i32) * (x as i32 - bx as i32);
            let d2 = (ax as i32 - cx as i32) * (y as i32 - cy as i32)
                - (ay as i32 - cy as i32) * (x as i32 - cx as i32);
            let inside = (d0 >= 0 && d1 >= 0 && d2 >= 0) || (d0 <= 0 && d1 <= 0 && d2 <= 0);
            dst[y * sz + x] = if inside { fg } else { bg };
        }
    }
}
