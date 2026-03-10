use crate::backends::Backend;
use crate::Pixel;
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
    // Owns the creator so that the texture lifetime is self-contained.
    _creator: Box<TextureCreator<WindowContext>>,
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
                .create_texture_streaming(PixelFormatEnum::RGB565, width, height)
                .map_err(|e| e.to_string())?;
            std::mem::transmute::<Texture<'_>, Texture<'static>>(t)
        };
        Ok(Sdl2Backend {
            canvas,
            texture,
            event_pump,
            width: width as usize,
            height: height as usize,
            _creator: creator,
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
        self.render(&mut |pixels, _stride| pixels.fill(color));
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel) {
        self.render(&mut |pixels, stride| {
            for row in y..y + h {
                let start = row * stride + x;
                pixels[start..start + w].fill(color);
            }
        });
    }

    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize)) {
        self.texture
            .with_lock(None, |buffer: &mut [u8], stride: usize| {
                let pixel_stride = stride / 2;
                let pixels = unsafe {
                    std::slice::from_raw_parts_mut(
                        buffer.as_mut_ptr() as *mut Pixel,
                        buffer.len() / 2,
                    )
                };
                draw_fn(pixels, pixel_stride);
            })
            .expect("texture lock failed");
        self.canvas
            .copy(&self.texture, None, None)
            .expect("canvas copy failed");
        self.canvas.present();
    }

    fn poll_quit(&mut self) -> bool {
        for event in self.event_pump.poll_iter() {
            if let sdl2::event::Event::Quit { .. } = event {
                return true;
            }
        }
        false
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
    Box::new(Sdl2Backend::new(canvas, event_pump, w as u32, h as u32).expect("SDL2 backend"))
}
