use super::Backend;
use crate::{InputEvent, Pixel};

extern "C" {
    fn imgrids_blit(ptr: *const u8, byte_len: usize, width: u32, height: u32);
    fn imgrids_setup_input();
    fn imgrids_next_event(
        out_type: *mut i32,
        out_x: *mut i32,
        out_y: *mut i32,
    ) -> i32;
    fn emscripten_sleep(ms: u32);
}

pub struct WebBackend {
    pixels: Vec<Pixel>,
    width: usize,
    height: usize,
    events: Vec<InputEvent>,
}

impl Backend for WebBackend {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }

    fn clear(&mut self, color: Pixel) {
        self.pixels.fill(color);
    }

    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Pixel,
    ) {
        for row in y..y + h {
            let start = row * self.width + x;
            self.pixels[start..start + w].fill(color);
        }
    }

    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize)) {
        draw_fn(&mut self.pixels, self.width);
        unsafe {
            imgrids_blit(
                self.pixels.as_ptr() as *const u8,
                self.pixels.len() * std::mem::size_of::<Pixel>(),
                self.width as u32,
                self.height as u32,
            );
        }
    }

    fn poll_events(&mut self) -> &[InputEvent] {
        self.events.clear();
        loop {
            let (mut t, mut x, mut y) = (0i32, 0i32, 0i32);
            if unsafe { imgrids_next_event(&mut t, &mut x, &mut y) } == 0 {
                break;
            }
            let ev = match t {
                0 => InputEvent::Press {
                    x: x as usize,
                    y: y as usize,
                },
                1 => InputEvent::Release {
                    x: x as usize,
                    y: y as usize,
                },
                2 => InputEvent::Move {
                    x: x as usize,
                    y: y as usize,
                },
                _ => continue,
            };
            self.events.push(ev);
        }
        &self.events
    }
}

pub fn init(w: usize, h: usize) -> Box<dyn Backend> {
    unsafe { imgrids_setup_input() };
    Box::new(WebBackend {
        pixels: vec![0; w * h],
        width: w,
        height: h,
        events: Vec::new(),
    })
}

pub fn sleep(ms: u32) {
    unsafe { emscripten_sleep(ms) };
}

pub fn run(
    mut backend: Box<dyn Backend>,
    mut tick_fn: impl FnMut(&mut dyn Backend),
) {
    loop {
        tick_fn(&mut *backend);
        sleep(0);
    }
}
