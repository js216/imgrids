use super::Backend;
use crate::{InputEvent, Pixel};
use std::cell::RefCell;

extern "C" {
    fn imgrids_blit(ptr: *const u8, byte_len: usize, width: u32, height: u32);
    fn imgrids_setup_input();
    fn imgrids_next_event(
        out_type: *mut i32,
        out_x: *mut i32,
        out_y: *mut i32,
    ) -> i32;
    fn emscripten_set_main_loop(func: extern "C" fn(), fps: i32, simulate_infinite_loop: i32);
    fn emscripten_sleep(ms: u32);
}

pub struct WebBackend {
    pixels: Vec<Pixel>,
    width: usize,
    height: usize,
    dirty: bool,
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
        self.dirty = true;
    }

    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: Pixel,
    ) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        if x >= x_end || y >= y_end { return; }
        let w = x_end - x;
        for row in y..y_end {
            let start = row * self.width + x;
            self.pixels[start..start + w].fill(color);
        }
        self.dirty = true;
    }

    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize)) {
        draw_fn(&mut self.pixels, self.width);
        self.dirty = true;
    }

    fn flush(&mut self) {
        if !self.dirty { return; }
        self.dirty = false;
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
        // Over-allocate by 64 rows so glyph blits near the bottom edge
        // can never index out of bounds.  Extra pixels are never uploaded.
        pixels: vec![0; w * (h + 64)],
        width: w,
        height: h,
        dirty: false,
        events: Vec::new(),
    })
}

pub fn sleep(ms: u32) {
    unsafe { emscripten_sleep(ms) };
}

thread_local! {
    static MAIN_LOOP_CB: RefCell<Option<Box<dyn FnMut()>>> = RefCell::new(None);
}

extern "C" fn main_loop_trampoline() {
    MAIN_LOOP_CB.with(|cb| {
        if let Some(f) = cb.borrow_mut().as_mut() {
            f();
        }
    });
}

pub fn run(
    mut backend: Box<dyn Backend>,
    mut tick_fn: impl FnMut(&mut dyn Backend) + 'static,
) {
    MAIN_LOOP_CB.with(|cb| {
        *cb.borrow_mut() = Some(Box::new(move || {
            tick_fn(&mut *backend);
        }));
    });
    // fps=30 matches the native sleep(33) rate; uses requestAnimationFrame
    // internally — no Asyncify overhead per frame.
    unsafe { emscripten_set_main_loop(main_loop_trampoline, 30, 1); }
}
