use imgrids::fonts::font_vga16::FONT;
use imgrids::raster::RasterAtlas;
use imgrids::InputEvent;

const WHITE: (u8, u8, u8) = (255, 255, 255);
const BLACK: (u8, u8, u8) = (0, 0, 0);
const BLUE:  (u8, u8, u8) = (0, 0, 200);

fn main() {
    let mut backend = init_backend(800, 480);

    let font = RasterAtlas::new(&FONT, 16, 32, Pixel::from_rgb(WHITE.0, WHITE.1, WHITE.2), Pixel::from_rgb(BLACK.0, BLACK.1, BLACK.2));

    let mut x0 = 0;
    let mut y0 = 0;

    backend.blit(&font, 100, 100, "Hello, world!");
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
            backend.fill_rect(x0, y0, 10, 10, Pixel::from_rgb(BLUE.0, BLUE.1, BLUE.2));
            backend.flush();
        }

        imgrids::sleep(33);
    }
}
