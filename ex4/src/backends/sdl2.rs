use crate::Pixel;
use crate::backends::Backend;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use sdl2::pixels::PixelFormatEnum;
use sdl2::EventPump;

pub struct Sdl2Backend<'a> {
    pub canvas: Canvas<Window>,
    pub texture: Texture<'a>,
    pub event_pump: EventPump,
    pub width: usize,
    pub height: usize,
}

impl<'a> Sdl2Backend<'a> {
    pub fn new(
        creator: &'a TextureCreator<WindowContext>,
        canvas: Canvas<Window>,
        event_pump: EventPump,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let texture = creator
            .create_texture_streaming(PixelFormatEnum::RGB565, width, height)
            .map_err(|e| e.to_string())?;

        Ok(Sdl2Backend {
            canvas,
            texture,
            event_pump,
            width: width as usize,
            height: height as usize,
        })
    }
}

impl<'a> Backend for Sdl2Backend<'a> {
    fn width(&self)  -> usize { self.width }
    fn height(&self) -> usize { self.height }

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
        self.texture.with_lock(None, |buffer: &mut [u8], stride: usize| {
            let pixel_stride = stride / 2; // RGB565 is 2 bytes per pixel
            let pixels = unsafe {
                std::slice::from_raw_parts_mut(
                    buffer.as_mut_ptr() as *mut Pixel,
                    buffer.len() / 2,
                )
            };
            draw_fn(pixels, pixel_stride);
        }).expect("texture lock failed");

        self.canvas.copy(&self.texture, None, None).expect("canvas copy failed");
        self.canvas.present();
    }
}
