mod backend;
pub use backend::Backend;

#[cfg(feature = "fb0")]
pub mod framebuffer;
#[cfg(feature = "fb0")]
pub use framebuffer::init;

#[cfg(feature = "sdl")]
pub mod sdl2;
#[cfg(feature = "sdl")]
pub use sdl2::init;
