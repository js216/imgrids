use crate::{Backend, InputEvent, Pixel};
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
    // Shadow pixel buffer: fill_rect/render write here; flush uploads once.
    pixels: Vec<Pixel>,
    dirty: bool,
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
            dirty: false,
            _creator: creator,
            events: Vec::new(),
        })
    }
}

impl Backend for Sdl2Backend {
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
        let width = self.width;
        draw_fn(&mut self.pixels, width);
        self.dirty = true;
    }

    fn flush(&mut self) {
        if !self.dirty { return; }
        self.dirty = false;
        let pixels = &self.pixels;
        let width = self.width;
        let height = self.height;
        self.texture
            .with_lock(None, |buffer: &mut [u8], stride: usize| {
                let row_bytes = width * 2;
                let src_bytes = width * height * 2;
                let src = unsafe {
                    std::slice::from_raw_parts(pixels.as_ptr() as *const u8, src_bytes)
                };
                if stride == row_bytes {
                    buffer[..src_bytes].copy_from_slice(src);
                } else {
                    for row in 0..buffer.len() / stride {
                        buffer[row * stride..row * stride + row_bytes]
                            .copy_from_slice(&src[row * row_bytes..(row + 1) * row_bytes]);
                    }
                }
            })
            .expect("texture lock failed");
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
