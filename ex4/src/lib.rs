pub mod backends;
pub mod fonts;
pub mod layout;
pub mod renderer;
pub mod renderers;

pub type Pixel = u16;

pub use layout::{resolve, Cell, Node};
pub use renderer::Renderer;

#[cfg(feature = "fb0")]
pub use crate::backends::framebuffer::Framebuf;

#[cfg(feature = "sdl")]
pub use crate::backends::sdl2::Sdl2Backend;
