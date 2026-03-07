/// A packed 32-bit ARGB pixel: 0xAARRGGBB.
pub type Pixel = u32;

pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Pixel {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub const fn rgb(r: u8, g: u8, b: u8) -> Pixel {
    argb(0xFF, r, g, b)
}

// Palette — same values as demo.c
pub const RED: Pixel = rgb(0xE6, 0x39, 0x46);
pub const GREEN: Pixel = rgb(0x52, 0xB7, 0x88);
pub const PURPLE: Pixel = rgb(0x6A, 0x4C, 0x93);
pub const BLUE: Pixel = rgb(0x48, 0x95, 0xEF);
pub const VIOLET: Pixel = rgb(0x9B, 0x5D, 0xE5);
pub const PINK: Pixel = rgb(0xF1, 0x5B, 0xB5);
pub const WHITE: Pixel = rgb(0xFF, 0xFF, 0xFF);
pub const GRAY: Pixel = rgb(0xAA, 0xAA, 0xAA);
pub const TEAL: Pixel = rgb(0x0D, 0x3B, 0x38);
pub const BROWN: Pixel = rgb(0x2C, 0x1A, 0x0E);
pub const OLIVE: Pixel = rgb(0x1E, 0x20, 0x10);
pub const SLATE: Pixel = rgb(0x0F, 0x15, 0x35);
pub const BLACK: Pixel = argb(0x00, 0x00, 0x00, 0x00); // transparent
