use crate::Backend;
use crate::Pixel;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::ptr;

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;
const FBIOBLANK: libc::c_ulong = 0x4611;
const FB_BLANK_UNBLANK: libc::c_int = 0;
const FB_BLANK_POWERDOWN: libc::c_int = 4;

#[repr(C)]
#[derive(Default)]
struct FbVarScreenInfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: [u32; 3],
    green: [u32; 3],
    blue: [u32; 3],
    transp: [u32; 3],
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    timing: [u32; 7],
    reserved: [u32; 4],
}

#[repr(C)]
#[derive(Default)]
struct FbFixScreenInfo {
    id: [u8; 16],
    smem_start: usize,
    smem_len: u32,
    type_: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: usize,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

pub struct Framebuf {
    /// Direct alias into the mmap — Pixel is the native hardware type (u16 RGB565).
    /// stride accounts for hardware line padding; always use it for row offsets.
    pub pixels: &'static mut [Pixel],
    pub stride: usize,
    pub width: usize,
    pub height: usize,
    fd: i32,
    mmap_ptr: *mut libc::c_void,
    mmap_size: usize,
}

impl Framebuf {
    pub fn open(path: &str) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("open {path}: {e}"))?;
        let fd = file.as_raw_fd();

        let mut vinfo = FbVarScreenInfo::default();
        let mut finfo = FbFixScreenInfo::default();
        unsafe {
            if libc::ioctl(fd, FBIOGET_VSCREENINFO, &mut vinfo) < 0 {
                return Err("ioctl FBIOGET_VSCREENINFO failed".into());
            }
            if libc::ioctl(fd, FBIOGET_FSCREENINFO, &mut finfo) < 0 {
                return Err("ioctl FBIOGET_FSCREENINFO failed".into());
            }
            if libc::ioctl(fd, FBIOBLANK, FB_BLANK_UNBLANK) < 0 {
                return Err("ioctl FBIOBLANK failed".into());
            }
        }

        let bpp = vinfo.bits_per_pixel as usize;
        let pixel_bytes = std::mem::size_of::<Pixel>();
        if bpp != pixel_bytes * 8 {
            return Err(format!(
                "framebuffer is {bpp}bpp but Pixel is {}bpp — recompile or run: fbset -depth {}",
                pixel_bytes * 8,
                pixel_bytes * 8,
            ));
        }

        let stride = finfo.line_length as usize / pixel_bytes;
        let mmap_size = finfo.smem_len as usize;

        let mmap_ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                mmap_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if mmap_ptr == libc::MAP_FAILED {
            return Err("mmap failed".into());
        }

        std::mem::forget(file);

        let pixels = unsafe {
            std::slice::from_raw_parts_mut(mmap_ptr as *mut Pixel, mmap_size / pixel_bytes)
        };

        Ok(Framebuf {
            pixels,
            stride,
            width: vinfo.xres as usize,
            height: vinfo.yres as usize,
            fd,
            mmap_ptr,
            mmap_size,
        })
    }
}

impl Backend for Framebuf {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }

    #[inline]
    fn clear(&mut self, color: Pixel) {
        self.pixels.fill(color);
    }

    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Pixel) {
        for row in y..y + h {
            let start = row * self.stride + x;
            self.pixels[start..start + w].fill(color);
        }
    }

    fn render(&mut self, draw_fn: &mut dyn FnMut(&mut [Pixel], usize)) {
        draw_fn(self.pixels, self.stride);
    }
}

impl Drop for Framebuf {
    fn drop(&mut self) {
        unsafe {
            libc::ioctl(self.fd, FBIOBLANK, FB_BLANK_POWERDOWN);
            libc::munmap(self.mmap_ptr, self.mmap_size);
            libc::close(self.fd);
        }
    }
}

pub fn init(_w: usize, _h: usize) -> Box<dyn super::Backend> {
    Box::new(Framebuf::open("/dev/fb0").expect("open framebuffer"))
}
