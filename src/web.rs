use super::Backend;
use crate::Pixel;

extern "C" {
    fn imgrids_blit(ptr: *const u8, byte_len: usize, width: u32, height: u32);
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
}

pub fn init(w: usize, h: usize) -> Box<dyn Backend> {
    Box::new(WebBackend {
        pixels: vec![0; w * h],
        width: w,
        height: h,
    })
}

/// Hand the main loop to the browser's requestAnimationFrame.
///
/// `draw_fn` must be `'static` because it lives for the duration of the page.
/// This function never returns.
pub fn run(
    backend: Box<dyn Backend>,
    draw_fn: impl FnMut(&mut [Pixel], usize) + 'static,
) -> ! {
    struct State {
        backend: Box<dyn Backend>,
        draw_fn: Box<dyn FnMut(&mut [Pixel], usize)>,
    }

    extern "C" fn tick(arg: *mut libc::c_void) {
        let s = unsafe { &mut *(arg as *mut State) };
        // Split borrow: backend and draw_fn are separate fields.
        let draw_fn = &mut *s.draw_fn as *mut dyn FnMut(&mut [Pixel], usize);
        s.backend.render(unsafe { &mut *draw_fn });
    }

    let state = Box::into_raw(Box::new(State {
        backend,
        draw_fn: Box::new(draw_fn),
    }));

    unsafe {
        emscripten_set_main_loop_arg(tick, state as *mut libc::c_void, 0, 1);
    }

    unreachable!()
}
