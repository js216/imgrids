#!/usr/bin/env python3
"""
font_to_bin.py — render ASCII 32–126 from a TTF and emit a binary font file.

Usage:
    python3 font_to_bin.py Roboto.ttf 12 > Roboto.font
    # or as a Makefile rule:
    # %.font: %.ttf
    #     python3 font_to_bin.py $< 12 > $@

Binary format (all values little-endian uint32):
    [4]  cell_w       — fixed cell width (widest glyph, all others padded)
    [4]  cell_h       — cell height = font_size * 1.5
    [128 * cell_w * cell_h]  alpha masks, one per ASCII slot (0=bg, 255=fg)
                             slots 0-31 and 127 are zeroed (unused)

The receiver composites each mask against its chosen fg+bg colours once at
startup; the main loop then does nothing but memcpy.
"""

import sys, struct, math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

if len(sys.argv) != 3:
    print(f"Usage: {sys.argv[0]} <font.ttf> <font_size>", file=sys.stderr)
    sys.exit(1)

ttf_path  = sys.argv[1]
font_size = int(sys.argv[2])

CELL_H      = int(font_size * 1.5)
SUPERSAMPLE = 4
render_size = font_size  * SUPERSAMPLE
render_h    = CELL_H     * SUPERSAMPLE

ASCII_FIRST = 32
ASCII_LAST  = 126

try:
    font = ImageFont.truetype(ttf_path, render_size)
except OSError as e:
    print(f"ERROR: {e}", file=sys.stderr)
    sys.exit(1)

def render_glyph_alpha(char, out_w, out_h):
    """Render char at supersample resolution, downsample to out_w x out_h,
    return list of out_w*out_h uint8 alpha values (row-major)."""
    advance_render = max(1, math.ceil(font.getlength(char)))
    img = Image.new("L", (advance_render, render_h), 0)
    ImageDraw.Draw(img).text((0, 0), char, font=font, fill=255)
    small = img.resize((out_w, out_h), Image.LANCZOS)
    return list(small.getdata())

# --- measure widest glyph to determine CELL_W ---
cell_w = 1
for code in range(ASCII_FIRST, ASCII_LAST + 1):
    w = max(1, round(math.ceil(font.getlength(chr(code))) / SUPERSAMPLE))
    if w > cell_w:
        cell_w = w

# --- write header ---
out = sys.stdout.buffer
out.write(struct.pack('<II', cell_w, CELL_H))

# --- write 128 glyph slots ---
blank = bytes(cell_w * CELL_H)

for code in range(128):
    if ASCII_FIRST <= code <= ASCII_LAST:
        pixels = render_glyph_alpha(chr(code), cell_w, CELL_H)
        out.write(bytes(pixels))
    else:
        out.write(blank)
