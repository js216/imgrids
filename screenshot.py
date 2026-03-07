#!/usr/bin/env python3
import sys
from PIL import Image

FBSET  = "/sys/class/graphics/fb0"
WIN_W  = 800
WIN_H  = 480

def sysfs(name):
    with open(f"{FBSET}/{name}") as f:
        return f.read().strip()

fb_h   = int(sysfs("virtual_size").split(",")[1])
stride = int(sysfs("stride"))
Bpp    = int(sysfs("bits_per_pixel")) // 8
row_px = stride // Bpp

with open("/dev/fb0", "rb") as f:
    raw = f.read(stride * fb_h)

# Parse as full-width RGB, ignoring the X byte, then crop
img = Image.frombytes("RGB", (row_px, fb_h), raw, "raw", "BGRX")
img = img.crop((0, 0, WIN_W, WIN_H))

out = sys.argv[1] if len(sys.argv) > 1 else "screenshot.png"
img.save(out)
print(f"saved {WIN_W}x{WIN_H} → {out}")
