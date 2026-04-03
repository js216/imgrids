#[cfg(feature = "sdl")]
use imgrids::Rgb565;
#[cfg(feature = "sdl")]
type Pixel = Rgb565;

#[cfg(feature = "fb0")]
use imgrids::Rgb888;
#[cfg(feature = "fb0")]
type Pixel = Rgb888;

#[cfg(feature = "web")]
use imgrids::Rgba8888;
#[cfg(feature = "web")]
type Pixel = Rgba8888;

use imgrids::fonts::font_vga16::FONT;
use imgrids::raster::RasterAtlas;
use imgrids::ttf::TtfAtlas;
use imgrids::{rgb, InputEvent};

const WHITE: (u8, u8, u8) = (255, 255, 255);
const BLACK: (u8, u8, u8) = (0, 0, 0);
const BLUE:  (u8, u8, u8) = (0, 0, 200);

fn main() {
    #[cfg(feature = "sdl")]
    let mut backend = imgrids::sdl2::init(800, 480);
    #[cfg(feature = "fb0")]
    let mut backend = imgrids::framebuffer::init(800, 480);
    #[cfg(feature = "web")]
    let mut backend = imgrids::web::init(800, 480);

    let font = RasterAtlas::new(&FONT, 16, 32, rgb!(WHITE), rgb!(BLACK));
    let ttf = TtfAtlas::new(&[("fonts/MyriadPro-Regular.ttf", 32)], &[], rgb!(WHITE), rgb!(BLACK))
        .expect("MyriadPro-Regular.ttf");

    let mut x0 = 0;
    let mut y0 = 0;

    backend.blit(&font, 100, 100, "Hello,");
    backend.blit(&ttf, 100, 132, "world!");
    backend.flush();

    loop {
        for ev in backend.poll_events().iter().copied() {
            match ev {
                InputEvent::Quit => std::process::exit(0),
                InputEvent::Press {x, y} => {x0 = x; y0 = y;}
                InputEvent::Release {..} => {x0 = 0; y0 = 0;}
                InputEvent::Move {x, y}  => {x0 = x; y0 = y;}
            }
        }

        if (x0 != 0) && (y0 != 0) {
            backend.fill_rect(x0, y0, 10, 10, rgb!(BLUE));
            backend.flush();
        }

        imgrids::sleep(33);
    }
}
