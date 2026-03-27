use crate::{Backend, InputEvent, Pixel, Renderer};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use sdl2::EventPump;

pub struct Sdl2Backend {
    pub canvas: Canvas<Window>,
    pub texture: Texture<'static>,
    pub event_pump: EventPump,
    pub width: usize,
    pub height: usize,
    // Shadow pixel buffer: fill_rect/blit write here; flush uploads once.
    pixels: Vec<Pixel>,
    // Dirty bounding rectangle since last flush.
    dirty_x0: usize,
    dirty_y0: usize,
    dirty_x1: usize,
    dirty_y1: usize,
    // Reusable scratch buffer for sub-rect texture uploads.
    upload_buf: Vec<u8>,
    // Owns the creator so that the texture lifetime is self-contained.
    _creator: Box<TextureCreator<WindowContext>>,
    events: Vec<InputEvent>,
}

impl Sdl2Backend {
    pub fn new(
        canvas: Canvas<Window>,
        event_pump: EventPump,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let creator = Box::new(canvas.texture_creator());
        // SAFETY: the texture borrows from `creator`, which we keep pinned
        // inside the same struct and never move or drop before `texture`.
        let texture = unsafe {
            let t = (&*creator as &TextureCreator<WindowContext>)
                .create_texture_streaming(
                    PixelFormatEnum::RGB565,
                    width,
                    height,
                )
                .map_err(|e| e.to_string())?;
            std::mem::transmute::<Texture<'_>, Texture<'static>>(t)
        };
        Ok(Sdl2Backend {
            canvas,
            texture,
            event_pump,
            width: width as usize,
            height: height as usize,
            // Over-allocate by 64 rows so glyph blits near the bottom edge
            // can never index out of bounds.  Extra pixels are never uploaded.
            pixels: vec![0; (width * (height + 64)) as usize],
            dirty_x0: width as usize,
            dirty_y0: height as usize,
            dirty_x1: 0,
            dirty_y1: 0,
            upload_buf: Vec::new(),
            _creator: creator,
            events: Vec::new(),
        })
    }

    #[inline]
    fn mark_dirty(&mut self, x: usize, y: usize, x_end: usize, y_end: usize) {
        if x < self.dirty_x0 { self.dirty_x0 = x; }
        if y < self.dirty_y0 { self.dirty_y0 = y; }
        if x_end > self.dirty_x1 { self.dirty_x1 = x_end; }
        if y_end > self.dirty_y1 { self.dirty_y1 = y_end; }
    }
}

impl Backend for Sdl2Backend {
    fn clear(&mut self, color: Pixel) {
        self.pixels.fill(color);
        self.dirty_x0 = 0;
        self.dirty_y0 = 0;
        self.dirty_x1 = self.width;
        self.dirty_y1 = self.height;
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
        self.mark_dirty(x, y, x_end, y_end);
    }

    fn blit(&mut self, atlas: &dyn Renderer, x: usize, y: usize, text: &str) -> usize {
        let width = self.width;
        let end_x = atlas.blit(&mut self.pixels, width, x, y, text);
        let h = atlas.cell_height();
        self.mark_dirty(x, y, end_x, y + h);
        end_x
    }

    fn flush(&mut self) {
        let x0 = self.dirty_x0;
        let y0 = self.dirty_y0;
        let x1 = self.dirty_x1.min(self.width);
        let y1 = self.dirty_y1.min(self.height);
        if x0 >= x1 || y0 >= y1 { return; }
        self.dirty_x0 = self.width;
        self.dirty_y0 = self.height;
        self.dirty_x1 = 0;
        self.dirty_y1 = 0;

        let width = self.width;
        let row_bytes = width * 2;
        let rect_w = x1 - x0;
        let rect_h = y1 - y0;
        let rect = sdl2::rect::Rect::new(x0 as i32, y0 as i32, rect_w as u32, rect_h as u32);
        // Upload the dirty rect. When the rect spans the full width,
        // we can point directly into the pixel buffer (contiguous rows).
        // Otherwise, gather the rect rows into a scratch buffer.
        let rect_row_bytes = rect_w * 2;
        let src = unsafe {
            std::slice::from_raw_parts(
                self.pixels.as_ptr() as *const u8,
                self.pixels.len() * 2,
            )
        };
        if x0 == 0 && x1 == width {
            let off = y0 * row_bytes;
            self.texture.update(rect, &src[off..off + rect_h * row_bytes], row_bytes)
                .expect("texture update failed");
        } else {
            self.upload_buf.resize(rect_h * rect_row_bytes, 0);
            for row in 0..rect_h {
                let src_off = (y0 + row) * row_bytes + x0 * 2;
                let dst_off = row * rect_row_bytes;
                self.upload_buf[dst_off..dst_off + rect_row_bytes]
                    .copy_from_slice(&src[src_off..src_off + rect_row_bytes]);
            }
            self.texture.update(rect, &self.upload_buf[..rect_h * rect_row_bytes], rect_row_bytes)
                .expect("texture update failed");
        }
        self.canvas
            .copy(&self.texture, None, None)
            .expect("canvas copy failed");
        self.canvas.present();
    }

    fn poll_events(&mut self) -> &[InputEvent] {
        self.events.clear();
        for event in self.event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => {
                    self.events.push(InputEvent::Quit);
                }
                sdl2::event::Event::MouseButtonDown {
                    x,
                    y,
                    mouse_btn: sdl2::mouse::MouseButton::Left,
                    ..
                } => {
                    self.events.push(InputEvent::Press {
                        x: x as usize,
                        y: y as usize,
                    });
                }
                sdl2::event::Event::MouseButtonUp {
                    x,
                    y,
                    mouse_btn: sdl2::mouse::MouseButton::Left,
                    ..
                } => {
                    self.events.push(InputEvent::Release {
                        x: x as usize,
                        y: y as usize,
                    });
                }
                sdl2::event::Event::MouseMotion {
                    x, y, mousestate, ..
                } if mousestate.left() => {
                    self.events.push(InputEvent::Move {
                        x: x as usize,
                        y: y as usize,
                    });
                }
                _ => {}
            }
        }
        &self.events
    }
}

pub fn init(w: usize, h: usize) -> Box<dyn Backend> {
    let sdl = sdl2::init().expect("SDL2 init");
    let video = sdl.video().expect("SDL2 video");
    let window = video
        .window("imgrids demo", w as u32, h as u32)
        .position_centered()
        .build()
        .expect("window");
    let canvas = window.into_canvas().build().expect("canvas");
    let event_pump = sdl.event_pump().expect("event pump");
    Box::new(
        Sdl2Backend::new(canvas, event_pump, w as u32, h as u32)
            .expect("SDL2 backend"),
    )
}
