use imgrids::{
    Pixel, Framebuf,
    backends::Backend,
    layout::{cell, col, resolve, row},
    renderers::{CharsAtlas, MonoAtlas, MonoFont, ShapesAtlas, TtAtlas, TtFont},
};

use imgrids::fonts::font8x8::FONT as FONT_8X8;
use imgrids::fonts::font_terminus_8x16::FONT as FONT_TER;
use imgrids::fonts::font_vga16::FONT as FONT_VGA;

const WHITE:    Pixel = 0xFFFF;
const BLACK:    Pixel = 0x0000;
const RED:      Pixel = 0xF800;
const GREEN:    Pixel = 0x07E0;
const BLUE:     Pixel = 0x001F;
const ROSE:     Pixel = 0xFCB6;
const MINT:     Pixel = 0x97F6;
const SKY:      Pixel = 0x9E3F;
const LAVENDER: Pixel = 0xCCBF;

// ─── Geometry ────────────────────────────────────────────────────────────────

const SCREEN_W: usize = 800;
const SCREEN_H: usize = 480;

const WIN_X:    usize = 40;
const WIN_Y:    usize = 40;
const WIN_W:    usize = 720;
const WIN_H:    usize = 400;

const MARGIN_X: usize = 20;
const MARGIN_Y: usize = 20;
const BORDER:   usize = 4;

// ─── Text generators ─────────────────────────────────────────────────────────

fn gen_hello() -> &'static str { "Hello!    " }
fn gen_world() -> &'static str { "World!    " }

fn gen_random() -> &'static str {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0x853C49E6748FEA9B);
    let mut s = STATE.load(Ordering::Relaxed);
    s ^= s << 13; s ^= s >> 7; s ^= s << 17;
    STATE.store(s, Ordering::Relaxed);
    thread_local! { static BUF: std::cell::RefCell<[u8; 10]> = std::cell::RefCell::new([0u8; 10]); }
    BUF.with(|b| {
        let mut buf = b.borrow_mut();
        let mut v = s;
        for i in 0..10 { buf[i] = (32 + (v & 0x5F) as u8) % 95 + 32; v >>= 6; }
        let s_slice = unsafe { std::str::from_utf8_unchecked(&*buf) };
        unsafe { std::mem::transmute::<&str, &'static str>(s_slice) }
    })
}

fn main() {
    let mut fb = Framebuf::open("/dev/fb0").expect("open framebuffer");

    // Atlases
    let ch1 = CharsAtlas::new(&FONT_VGA, 16, 32, WHITE, BLACK);
    let ch2 = CharsAtlas::new(&FONT_VGA, 32, 64, RED, BLACK);
    let ch3 = CharsAtlas::new(&FONT_TER, 16, 32, GREEN, BLACK);
    let ch4 = CharsAtlas::new(&FONT_TER, 32, 64, BLUE, BLACK);
    let ch5 = CharsAtlas::new(&FONT_8X8,  8, 16, MINT, BLACK);
    let ch6 = CharsAtlas::new(&FONT_8X8, 16, 32, SKY, BLACK);
    let roboto = MonoFont::load("RobotoMono-Regular.ttf").expect("font");
    let myriad = TtFont::load("MyriadPro-Regular.ttf").expect("font");
    let ch7 = roboto.at(32, ROSE, BLACK);
    let ch8 = myriad.at(32, LAVENDER, BLACK);

    // Layout
    let layout = row(1, vec![
        col(1, vec![
            cell(ch1.as_renderer(), gen_hello),
            cell(ch2.as_renderer(), gen_world),
            cell(ch3.as_renderer(), gen_random),
            cell(ch4.as_renderer(), gen_random),
            cell(ch5.as_renderer(), gen_hello),
            cell(ch6.as_renderer(), gen_world),
            cell(ch7.as_renderer(), gen_random),
            cell(ch8.as_renderer(), gen_random),
        ]),
    ]);

    let mut cells = resolve(
        &layout,
        WIN_X + MARGIN_X,
        WIN_Y + MARGIN_Y,
        WIN_W - 2 * MARGIN_X,
        WIN_H - 2 * MARGIN_Y,
    );

    fb.draw_border(WIN_X, WIN_Y, WIN_W, WIN_H, BORDER, WHITE);

    loop {
        for c in &mut cells {
            c.draw(fb.pixels, fb.stride);
        }
        std::thread::sleep(std::time::Duration::from_micros(33_333));
    }
}

trait AsRenderer { fn as_renderer(&self) -> &dyn imgrids::Renderer; }
macro_rules! impl_as_renderer { ($t:ty) => { impl AsRenderer for $t { fn as_renderer(&self) -> &dyn imgrids::Renderer { self } } }; }
impl_as_renderer!(ShapesAtlas); impl_as_renderer!(CharsAtlas); impl_as_renderer!(MonoAtlas); impl_as_renderer!(TtAtlas);
