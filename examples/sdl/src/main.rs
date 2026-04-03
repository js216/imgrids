use imgrids::Rgb565;
pub type Pixel = Rgb565;
#[allow(dead_code)] mod ui { include!(concat!(env!("OUT_DIR"), "/ui.rs")); }
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_sdl::init(w, h) }
include!("../../app.rs");
