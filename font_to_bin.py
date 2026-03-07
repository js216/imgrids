#!/usr/bin/env python3
"""
font_to_bin.py — render ASCII 32–126 from a TTF and emit a binary font file.

Usage:
    python3 font_to_bin.py Roboto.ttf 12 > Roboto.font

Makefile rule:
    %.font: %.ttf
        python3 font_to_bin.py $< $(FONT_SIZE) > $@

Binary format (little-endian):
    [4]         cell_h              — shared height for all glyphs (= font_size * 1.5)
    [128 x 2]   advance_w[code]     — per-glyph width in pixels (uint16); 0 = unused slot
    per glyph:  advance_w[code] * cell_h bytes of uint8 alpha mask, packed
                sequentially for code 0..127.  Slots with advance_w==0 emit no bytes.
                alpha 0 = transparent background, 255 = full foreground ink.
"""

import sys, struct, math
from PIL import Image, ImageDraw, ImageFont

if len(sys.argv) != 3:
    print(f"Usage: {sys.argv[0]} <font.ttf> <font_size>", file=sys.stderr)
    sys.exit(1)

ttf_path  = sys.argv[1]
font_size = int(sys.argv[2])

CELL_H      = int(font_size * 1.5)
SUPERSAMPLE = 4
render_size = font_size * SUPERSAMPLE
render_h    = CELL_H    * SUPERSAMPLE

ASCII_FIRST = 32
ASCII_LAST  = 126

try:
    font = ImageFont.truetype(ttf_path, render_size)
except OSError as e:
    print(f"ERROR: {e}", file=sys.stderr)
    sys.exit(1)

def render_glyph(char, out_w):
    """Render char at supersample resolution, downsample to out_w x CELL_H.
    Returns list of out_w * CELL_H uint8 alpha values (row-major)."""
    advance_render = max(1, math.ceil(font.getlength(char)))
    img = Image.new("L", (advance_render, render_h), 0)
    ImageDraw.Draw(img).text((0, 0), char, font=font, fill=255)
    small = img.resize((out_w, CELL_H), Image.LANCZOS)
    return list(small.getdata())

# Measure each glyph's natural width at output resolution
widths = [0] * 128
for code in range(ASCII_FIRST, ASCII_LAST + 1):
    widths[code] = max(1, round(math.ceil(font.getlength(chr(code))) / SUPERSAMPLE))

out = sys.stdout.buffer

# Header: cell_h, then 128 x uint16 advance widths
out.write(struct.pack('<I', CELL_H))
for w in widths:
    out.write(struct.pack('<H', w))

# Glyph data: only slots with advance_w > 0, packed sequentially
for code in range(128):
    if widths[code] > 0:
        pixels = render_glyph(chr(code), widths[code])
        out.write(bytes(pixels))
