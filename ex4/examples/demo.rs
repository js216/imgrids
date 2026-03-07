// Demo application — equivalent of demo.c.
//
// This file is the ONLY place that knows about the specific atlases, colours,
// layout, and text generators.  Swap a renderer by changing one line.
//
// Build:
//   cargo build --example demo --release
// Run (needs /dev/fb0):
//   sudo ./target/release/examples/demo

use imgrids::{
    color::*,
    framebuf::Framebuf,
    layout::{cell, col, resolve, row},
    renderers::{CharsAtlas, MonoAtlas, ShapesAtlas, TtAtlas},
};

// ─── Geometry ────────────────────────────────────────────────────────────────

const WIN_X: usize = 0;
const WIN_Y: usize = 0;
const SCREEN_W: usize = 800;
const SCREEN_H: usize = 480;
const MARGIN_X: usize = 20;
const MARGIN_Y: usize = 20;
const BORDER: usize = 4;

// ─── Text generators — called each frame ─────────────────────────────────────
//
// These must return `&'static str`.  For dynamic content (clocks, counters)
// use a thread-local or a global atomic.

fn gen_hello() -> &'static str {
    "Hello!    "
}
fn gen_world() -> &'static str {
    "World!    "
}

fn gen_random() -> &'static str {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0x853C49E6748FEA9B);

    let mut s = STATE.load(Ordering::Relaxed);
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    STATE.store(s, Ordering::Relaxed);

    thread_local! {
        static BUF: std::cell::RefCell<[u8; 10]> = std::cell::RefCell::new([0u8; 10]);
    }

    BUF.with(|b| {
        let mut buf = b.borrow_mut();
        let mut v = s;
        for i in 0..10 {
            buf[i] = (32 + (v & 0x5F) as u8) % 95 + 32;
            v >>= 6;
        }

        let s_slice = unsafe { std::str::from_utf8_unchecked(&*buf) };

        unsafe { std::mem::transmute::<&str, &'static str>(s_slice) }
    })
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let mut fb = Framebuf::open("/dev/fb0").expect("open framebuffer");

    if fb.width < WIN_X + SCREEN_W || fb.height < WIN_Y + SCREEN_H {
        eprintln!(
            "framebuffer too small: got {}×{}, need {}×{}",
            fb.width,
            fb.height,
            WIN_X + SCREEN_W,
            WIN_Y + SCREEN_H
        );
        return;
    }

    // ── Atlases ──────────────────────────────────────────────────────────────
    //
    // To swap a renderer: change the type on the left and the constructor
    // call on the right — the layout below is unaffected.

    let l_shapes = ShapesAtlas::new(24, RED, TEAL);
    let l_chars = CharsAtlas::new(24, 24, GREEN, BROWN);
    let l_mono =
        MonoAtlas::load("../ex2/RobotoMono-Regular-24.font", PURPLE, OLIVE).expect("l_mono");
    let l_tt = TtAtlas::load("../ex2/Roboto-Regular-24.font", BLUE, SLATE).expect("l_tt");

    let r_shapes = ShapesAtlas::new(24, VIOLET, TEAL);
    let r_chars = CharsAtlas::new(24, 24, PINK, BROWN);
    let r_mono =
        MonoAtlas::load("../ex2/RobotoMono-Regular-12.font", WHITE, OLIVE).expect("r_mono");
    let r_tt = TtAtlas::load("../ex2/Roboto-Regular-12.font", GRAY, SLATE).expect("r_tt");

    // ── Layout ───────────────────────────────────────────────────────────────
    //
    // Mirrors the C macro tree exactly.  The Renderer references are tied to
    // the atlas lifetimes above — the borrow checker enforces this.

    let layout = col(
        1,
        vec![
            row(
                1,
                vec![
                    cell(r_tt.as_renderer(), gen_hello),
                    cell(r_tt.as_renderer(), gen_world),
                    cell(r_tt.as_renderer(), gen_random),
                    cell(r_tt.as_renderer(), gen_random),
                    cell(r_tt.as_renderer(), gen_random),
                ],
            ),
            row(
                10,
                vec![
                    col(
                        1,
                        vec![
                            cell(l_shapes.as_renderer(), gen_hello),
                            cell(l_chars.as_renderer(), gen_hello),
                            cell(l_mono.as_renderer(), gen_hello),
                            cell(l_tt.as_renderer(), gen_hello),
                        ],
                    ),
                    col(
                        1,
                        vec![
                            cell(l_shapes.as_renderer(), gen_random),
                            cell(l_chars.as_renderer(), gen_random),
                            cell(l_mono.as_renderer(), gen_random),
                            cell(l_tt.as_renderer(), gen_random),
                        ],
                    ),
                    col(
                        1,
                        vec![
                            cell(r_shapes.as_renderer(), gen_world),
                            cell(r_chars.as_renderer(), gen_world),
                            cell(r_mono.as_renderer(), gen_world),
                            cell(r_tt.as_renderer(), gen_random),
                        ],
                    ),
                ],
            ),
        ],
    );

    // ── Resolve layout once ───────────────────────────────────────────────
    //
    // Call resolve() here (once) for a static layout.  To support dynamic
    // layouts — e.g. panels that resize — move this call inside the loop.

    let mut cells = resolve(
        &layout,
        WIN_X + MARGIN_X,
        WIN_Y + MARGIN_Y,
        SCREEN_W - 2 * MARGIN_X,
        SCREEN_H - 2 * MARGIN_Y,
    );

    // ── Draw border once ─────────────────────────────────────────────────

    fb.draw_border(WIN_X, WIN_Y, SCREEN_W, SCREEN_H, BORDER, WHITE);

    // ── Render loop ───────────────────────────────────────────────────────

    loop {
        for c in &mut cells {
            c.draw(fb.pixels, fb.stride);
        }
        // ~30 fps
        std::thread::sleep(std::time::Duration::from_micros(33_333));
    }
}

// ─── Convenience: make each atlas into a &dyn Renderer without extra imports ─

trait AsRenderer {
    fn as_renderer(&self) -> &dyn imgrids::Renderer;
}
macro_rules! impl_as_renderer {
    ($t:ty) => {
        impl AsRenderer for $t {
            fn as_renderer(&self) -> &dyn imgrids::Renderer {
                self
            }
        }
    };
}
impl_as_renderer!(ShapesAtlas);
impl_as_renderer!(CharsAtlas);
impl_as_renderer!(MonoAtlas);
impl_as_renderer!(TtAtlas);
