use imgrids::Rgba8888;
pub type Pixel = Rgba8888;
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_wasm::init(w, h) }
include!("../../raw.rs");
