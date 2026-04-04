use imgrids::Rgb888;
pub type Pixel = Rgb888;
mod ui;
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_fb0::init(w, h) }
include!("../../app.rs");
