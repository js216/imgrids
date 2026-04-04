use imgrids::Rgb565;
pub type Pixel = Rgb565;
mod ui;
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_fb0::init(w, h) }
include!("../../app.rs");
