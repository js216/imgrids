use imgrids::{Backend, InputEvent, Renderer, Rgba8888};

type P = Rgba8888;

extern "C" {
    fn imgrids_blit(ptr: *const u8, byte_len: usize, width: u32, height: u32);
    fn imgrids_setup_input();
    fn imgrids_next_event(
        out_type: *mut i32,
        out_x: *mut i32,
        out_y: *mut i32,
    ) -> i32;
}

pub struct WebBackend {
    pixels: Vec<P>,
    width: usize,
    height: usize,
    dirty: bool,
    events: Vec<InputEvent>,
}

impl Backend<P> for WebBackend {
    fn clear(&mut self, color: P) {
        self.pixels.fill(color);
        self.dirty = true;
    }

    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: P,
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

    fn blit(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, text: &str) -> usize {
        let width = self.width;
        let end_x = atlas.blit(&mut self.pixels, width, x, y, text);
        self.dirty = true;
        end_x
    }

    fn blit_char(&mut self, atlas: &dyn Renderer<P>, x: usize, y: usize, ch: char) -> usize {
        let width = self.width;
        let end_x = atlas.blit_char(&mut self.pixels, width, x, y, ch);
        self.dirty = true;
        end_x
    }

    fn blit_alpha(&mut self, icon: &imgrids::Icon, fg: P, bg: P) {
        imgrids::blit_alpha_buf(&mut self.pixels, self.width, icon, fg, bg);
        self.dirty = true;
    }

    fn flush(&mut self) {
        if !self.dirty { return; }
        self.dirty = false;
        unsafe {
            imgrids_blit(
                self.pixels.as_ptr() as *const u8,
                self.width * self.height * std::mem::size_of::<P>(),
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

pub fn init(w: usize, h: usize) -> Box<dyn Backend<P>> {
    unsafe { imgrids_setup_input() };
    Box::new(WebBackend {
        // Over-allocate by 64 rows so glyph blits near the bottom edge
        // can never index out of bounds.  Extra pixels are never uploaded.
        pixels: vec![P::default(); w * (h + 64)],
        width: w,
        height: h,
        dirty: false,
        events: Vec::new(),
    })
}

