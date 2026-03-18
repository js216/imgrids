use super::Backend;
use crate::{InputEvent, Pixel};

extern "C" {
    fn imgrids_blit(ptr: *const u8, byte_len: usize, width: u32, height: u32);
    fn imgrids_setup_input();
    fn imgrids_next_event(out_type: *mut i32, out_x: *mut i32, out_y: *mut i32) -> i32;
    fn emscripten_set_main_loop_arg(
        func: extern "C" fn(*mut libc::c_void),
        arg: *mut libc::c_void,
        fps: i32,
        simulate_infinite_loop: i32,
    );
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

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel) {
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
                    x: x as u32,
                    y: y as u32,
                },
                1 => InputEvent::Release {
                    x: x as u32,
                    y: y as u32,
                },
                2 => InputEvent::Move {
                    x: x as u32,
                    y: y as u32,
                },
                _ => continue,
            };
            self.events.push(ev);
        }
        &self.events
    }
}

pub fn init(w: usize, h: usize) -> Box<dyn Backend> {
    Box::new(WebBackend {
        pixels: vec![0; w * h],
        width: w,
        height: h,
        events: Vec::new(),
    })
}

/// Hand the main loop to the browser's requestAnimationFrame.
///
/// `tick_fn` receives a `&mut dyn Backend` each frame so it can call
/// `poll_events`, then `render`.  This function never returns.
pub fn run(backend: Box<dyn Backend>, tick_fn: impl FnMut(&mut dyn Backend)) -> ! {
    struct State {
        backend: Box<dyn Backend>,
        tick_fn: Box<dyn FnMut(&mut dyn Backend) + 'static>,
    }

    extern "C" fn tick(arg: *mut libc::c_void) {
        let s = unsafe { &mut *(arg as *mut State) };
        (s.tick_fn)(&mut *s.backend);
    }

    // SAFETY: emscripten_set_main_loop_arg with simulate_infinite_loop=1
    // unwinds the C stack via a JS exception without returning from or
    // dropping main().  main()'s locals therefore remain live for the entire
    // page lifetime, so any references captured by tick_fn are valid whenever
    // the callback fires.  Extending to 'static is sound under that invariant.
    let tick_fn: Box<dyn FnMut(&mut dyn Backend) + 'static> =
        unsafe { std::mem::transmute(Box::new(tick_fn) as Box<dyn FnMut(&mut dyn Backend)>) };

    let state = Box::into_raw(Box::new(State { backend, tick_fn }));

    unsafe {
        imgrids_setup_input();
        emscripten_set_main_loop_arg(tick, state as *mut libc::c_void, 0, 1);
    }

    unreachable!()
}
