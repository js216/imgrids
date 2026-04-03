use imgrids::Rgba8888;
pub type Pixel = Rgba8888;
#[allow(dead_code)] mod ui { include!(concat!(env!("OUT_DIR"), "/ui.rs")); }
fn init_backend(w: usize, h: usize) -> Box<dyn imgrids::Backend<Pixel>> { imgrids_wasm::init(w, h) }
include!("../../app.rs");
