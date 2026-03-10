#[cfg(all(feature = "bpp16", feature = "bpp32"))]
compile_error!("bpp16 and bpp32 are mutually exclusive");

#[cfg(not(any(feature = "bpp16", feature = "bpp32")))]
compile_error!("one of bpp16 or bpp32 must be selected");

#[cfg(feature = "bpp16")] pub type Pixel = u16;
#[cfg(feature = "bpp32")] pub type Pixel = u32;

#[cfg(feature = "bpp16")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        ((($r as u16 >> 3) << 11) | (($g as u16 >> 2) << 5) | ($b as u16 >> 3))
    }
}

#[cfg(feature = "bpp32")]
#[macro_export]
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        (($r as u32) << 16) | (($g as u32) << 8) | ($b as u32)
    }
}
