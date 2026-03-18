use imgrids::{
    layout::{cell, col, resolve, row},
    raster::RasterAtlas,
    rgb,
    ttf::TtfAtlas,
    Pixel,
};

use imgrids::fonts::font8x8::FONT as FONT_8X8;
use imgrids::fonts::font_terminus_8x16::FONT as FONT_TER;
use imgrids::fonts::font_vga16::FONT as FONT_VGA;

const WHITE: Pixel = rgb!(255, 255, 255);
const BLACK: Pixel = rgb!(0, 0, 0);
const RED: Pixel = rgb!(255, 0, 0);
const GREEN: Pixel = rgb!(0, 255, 0);
const BLUE: Pixel = rgb!(0, 0, 255);

// ─── Geometry ────────────────────────────────────────────────────────────────

const SCREEN_W: usize = 800;
const SCREEN_H: usize = 480;

const WIN_X: usize = 40;
const WIN_Y: usize = 40;
const WIN_W: usize = SCREEN_W - 2 * WIN_X;
const WIN_H: usize = SCREEN_H - 2 * WIN_Y;

const MARGIN_X: usize = 20;
const MARGIN_Y: usize = 20;
const BORDER: usize = 4;

// ─── Text generators ─────────────────────────────────────────────────────────

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
    thread_local! { static BUF: std::cell::RefCell<[u8; 10]> = std::cell::RefCell::new([0u8; 10]); }
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

fn main() {
    let mut backend = imgrids::init(SCREEN_W, SCREEN_H);

    // Atlases — leaked so their 'static references satisfy web::run's bound.
    let ch1 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_VGA, 16, 32, WHITE, BLACK)));
    let ch2 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_VGA, 32, 64, RED, BLACK)));
    let ch3 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_TER, 16, 32, GREEN, BLACK)));
    let ch4 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_TER, 32, 64, BLUE, BLACK)));
    let ch5 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_8X8, 8, 16, WHITE, BLACK)));
    let ch6 = &*Box::leak(Box::new(RasterAtlas::new(&FONT_8X8, 16, 32, RED, BLACK)));
    let ch7 = &*Box::leak(Box::new(TtfAtlas::new("fonts/RobotoMono-Regular.ttf", 32, GREEN, BLACK).expect("font")));
    let ch8 = &*Box::leak(Box::new(TtfAtlas::new("fonts/MyriadPro-Regular.ttf", 32, GREEN, BLACK).expect("font")));

    // Layout
    let layout = row(
        1,
        vec![
            col(
                1,
                vec![
                    cell(ch1.as_renderer(), gen_hello),
                    cell(ch2.as_renderer(), gen_world),
                    cell(ch3.as_renderer(), gen_random),
                    cell(ch4.as_renderer(), gen_random),
                    cell(ch5.as_renderer(), gen_hello),
                    cell(ch6.as_renderer(), gen_world),
                    cell(ch7.as_renderer(), gen_random),
                    cell(ch8.as_renderer(), gen_random),
                ],
            ),
            col(
                1,
                vec![
                    cell(ch1.as_renderer(), gen_hello),
                    cell(ch2.as_renderer(), gen_world),
                    cell(ch3.as_renderer(), gen_random),
                    cell(ch4.as_renderer(), gen_random),
                    cell(ch5.as_renderer(), gen_hello),
                    cell(ch6.as_renderer(), gen_world),
                    cell(ch7.as_renderer(), gen_random),
                    cell(ch8.as_renderer(), gen_random),
                ],
            ),
        ],
    );

    let mut cells = resolve(
        &layout,
        WIN_X + MARGIN_X,
        WIN_Y + MARGIN_Y,
        WIN_W - 2 * MARGIN_X,
        WIN_H - 2 * MARGIN_Y,
    );

    backend.draw_border(WIN_X, WIN_Y, WIN_W, WIN_H, BORDER, WHITE);

    #[cfg(target_os = "emscripten")]
    imgrids::web::run(backend, move |pixels, stride| {
        for c in &mut cells {
            c.draw(pixels, stride);
        }
    });

    #[cfg(not(target_os = "emscripten"))]
    loop {
        if backend.poll_quit() {
            break;
        }
        backend.render(&mut |pixels, stride| {
            for c in &mut cells {
                c.draw(pixels, stride);
            }
        });
        std::thread::sleep(std::time::Duration::from_micros(33_333));
    }
}

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
impl_as_renderer!(RasterAtlas);
impl_as_renderer!(TtfAtlas);
