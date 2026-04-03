use imgrids::Rgb565;
pub type Pixel = Rgb565;
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_sdl::init(w, h) }
include!("../../raw.rs");
