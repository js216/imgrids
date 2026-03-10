mod backend;
pub use backend::Backend;

#[cfg(feature = "fb0")]
pub mod framebuffer;

#[cfg(feature = "sdl")]
pub mod sdl2;
