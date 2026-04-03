use imgrids::{Backend, InputEvent, PixelFormat};
use std::fs::OpenOptions;
use std::mem::size_of;
use std::os::unix::io::AsRawFd;
use std::ptr;

// --- /dev/input/eventN constants ---------------------------------------------

/// `input_event` layout on 32-bit ARM Linux.
#[repr(C)]
struct RawInputEvent {
    tv_sec: i32,
    tv_usec: i32,
    type_: u16,
    code: u16,
    value: i32,
}

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const BTN_TOUCH: u16 = 0x14a;
const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;

struct TouchState {
    x: i32,
    y: i32,
    down: bool,
    prev_x: i32,
    prev_y: i32,
    prev_down: bool,
}

impl TouchState {
    const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            down: false,
            prev_x: 0,
            prev_y: 0,
            prev_down: false,
        }
    }
}

// --- Framebuffer constants ----------------------------------------------------

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

pub struct Framebuf<P: PixelFormat> {
    pub stride: usize,
    pub width: usize,
    pub height: usize,
    fd: i32,
    mmap_ptr: *mut libc::c_void,
    mmap_size: usize,
    input_fd: Option<i32>,
    touch: TouchState,
    events: Vec<InputEvent>,
    _pixel: std::marker::PhantomData<P>,
}

impl<P: PixelFormat> Framebuf<P> {
    /// Slice into the mmap'd pixel buffer.  Lifetime is tied to `&mut self`.
    fn pixels_mut(&mut self) -> &mut [P] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.mmap_ptr as *mut P,
                self.mmap_size / size_of::<P>(),
            )
        }
    }
}

impl<P: PixelFormat> Framebuf<P> {
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
        let pixel_bytes = size_of::<P>();
        if bpp != pixel_bytes * 8 {
            return Err(format!(
                "framebuffer is {bpp}bpp but Pixel is {}bpp - recompile or run: fbset -depth {}",
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

        let input_fd = unsafe {
            let path = b"/dev/input/event0\0";
            let ifd = libc::open(
                path.as_ptr() as *const libc::c_char,
                libc::O_RDONLY | libc::O_NONBLOCK,
            );
            if ifd < 0 {
                None
            } else {
                Some(ifd)
            }
        };

        Ok(Framebuf {
            stride,
            width: vinfo.xres as usize,
            height: vinfo.yres as usize,
            fd,
            mmap_ptr,
            mmap_size,
            input_fd,
            touch: TouchState::new(),
            events: Vec::new(),
            _pixel: std::marker::PhantomData,
        })
    }
}

impl<P: PixelFormat> Backend<P> for Framebuf<P> {
    #[inline]
    fn clear(&mut self, color: P) {
        self.pixels_mut().fill(color);
    }

    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        color: P,
    ) {
        let stride = self.stride;
        let pixels = self.pixels_mut();
        for row in y..y + h {
            let start = row * stride + x;
            pixels[start..start + w].fill(color);
        }
    }

    fn blit(&mut self, atlas: &dyn imgrids::Renderer<P>, x: usize, y: usize, text: &str) -> usize {
        let stride = self.stride;
        let pixels = self.pixels_mut();
        atlas.blit(pixels, stride, x, y, text)
    }


    fn blit_alpha(&mut self, icon: &imgrids::Icon, fg: P, bg: P) {
        let stride = self.stride;
        let pixels = self.pixels_mut();
        imgrids::blit_alpha_buf(pixels, stride, icon, fg, bg);
    }

    fn poll_events(&mut self) -> &[InputEvent] {
        self.events.clear();
        let fd = match self.input_fd {
            Some(fd) => fd,
            None => return &[],
        };
        loop {
            let mut raw = RawInputEvent {
                tv_sec: 0,
                tv_usec: 0,
                type_: 0,
                code: 0,
                value: 0,
            };
            let n = unsafe {
                libc::read(
                    fd,
                    &mut raw as *mut RawInputEvent as *mut libc::c_void,
                    size_of::<RawInputEvent>(),
                )
            };
            if n < size_of::<RawInputEvent>() as isize {
                break;
            }
            match (raw.type_, raw.code) {
                (EV_ABS, ABS_X) => self.touch.x = raw.value,
                (EV_ABS, ABS_Y) => self.touch.y = raw.value,
                (EV_KEY, BTN_TOUCH) => self.touch.down = raw.value != 0,
                (EV_SYN, SYN_REPORT) => {
                    let x = self.touch.x as usize;
                    let y = self.touch.y as usize;
                    let ev = match (self.touch.down, self.touch.prev_down) {
                        (true, false) => Some(InputEvent::Press { x, y }),
                        (false, true) => Some(InputEvent::Release { x, y }),
                        (true, true)
                            if self.touch.x != self.touch.prev_x
                                || self.touch.y != self.touch.prev_y =>
                        {
                            Some(InputEvent::Move { x, y })
                        }
                        _ => None,
                    };
                    if let Some(e) = ev {
                        self.events.push(e);
                    }
                    self.touch.prev_down = self.touch.down;
                    self.touch.prev_x = self.touch.x;
                    self.touch.prev_y = self.touch.y;
                }
                _ => {}
            }
        }
        &self.events
    }
}

impl<P: PixelFormat> Drop for Framebuf<P> {
    fn drop(&mut self) {
        unsafe {
            if let Some(ifd) = self.input_fd {
                libc::close(ifd);
            }
            libc::ioctl(self.fd, FBIOBLANK, FB_BLANK_POWERDOWN);
            libc::munmap(self.mmap_ptr, self.mmap_size);
            libc::close(self.fd);
        }
    }
}

pub fn init<P: PixelFormat>(w: usize, h: usize) -> Box<dyn imgrids::Backend<P>> {
    let fb: Framebuf<P> = Framebuf::open("/dev/fb0").expect("open framebuffer");
    assert!(
        fb.width == w && fb.height == h,
        "framebuffer is {}x{} but app expects {}x{}",
        fb.width, fb.height, w, h,
    );
    Box::new(fb)
}
