// imgrids/src/lib.rs
//
// Public re-exports so consumers only need `use imgrids::*`.

pub mod framebuf;
pub mod layout;
pub mod renderer;
pub mod renderers;

pub type Pixel = u16;

pub use framebuf::Framebuf;
pub use layout::{resolve, Cell, Node};
pub use renderer::Renderer;
