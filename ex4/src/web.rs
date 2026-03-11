use crate::Pixel;
use super::Backend;
use super::sdl2::Sdl2Backend;

extern "C" {
    fn emscripten_sleep(ms: u32);
}

pub struct WebBackend(Sdl2Backend);

impl Backend for WebBackend {
    fn width(&self)  -> usize { self.0.width()  }
    fn height(&self) -> usize { self.0.height() }

    fn clear(&mut self, color: Pixel) { self.0.clear(color) }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel) {
        self.0.fill_rect(x, y, w, h, color)
    }

    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize)) {
        self.0.render(draw_fn);
        unsafe { emscripten_sleep(1) };
    }

    fn poll_quit(&mut self) -> bool { self.0.poll_quit() }
}

pub fn init(w: usize, h: usize) -> Box<dyn Backend> {
    let sdl = sdl2::init().expect("SDL2 init");
    let video = sdl.video().expect("SDL2 video");

    // Force software rendering — no OpenGL/EGL needed for pixel blitting.
    sdl2::hint::set("SDL_RENDER_DRIVER", "software");

    let window = video
        .window("imgrids demo", w as u32, h as u32)
        .position_centered()
        .build()
        .expect("window");
    let canvas = window
        .into_canvas()
        .software()          // ← explicit software renderer
        .build()
        .expect("canvas");
    let event_pump = sdl.event_pump().expect("event pump");
    Box::new(WebBackend(
        Sdl2Backend::new(canvas, event_pump, w as u32, h as u32).expect("SDL2 backend")
    ))
}
