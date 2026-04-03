#!/usr/bin/env python3
"""Render an SVG to a binary alpha-mask file for use by the layout transpiler.

Usage: render_icon.py <input.svg> <height_px> <output.alpha>

Output format: 2 bytes LE width, 2 bytes LE height, then w*h alpha bytes
(0 = fully transparent / background, 255 = fully opaque / foreground).
"""

import struct
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image


def main():
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <input.svg> <height_px> <output.alpha>", file=sys.stderr)
        sys.exit(1)

    svg_path = sys.argv[1]
    target_h = int(sys.argv[2])
    out_path = sys.argv[3]

    # Rasterize SVG via ImageMagick convert
    with tempfile.NamedTemporaryFile(suffix=".png") as tmp:
        subprocess.run([
            "convert", svg_path,
            "-resize", f"x{target_h}",
            "-background", "none",
            "-flatten",
            "-colorspace", "Gray",
            "-depth", "8",
            tmp.name,
        ], check=True)
        img = Image.open(tmp.name).convert("L")

    w, h = img.size
    # Invert: SVG has dark fill on light background, but we need
    # high alpha = foreground (white icon on colored bg)
    from PIL import ImageOps
    img = ImageOps.invert(img)
    alpha = img.tobytes()

    Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "wb") as f:
        f.write(struct.pack("<HH", w, h))
        f.write(alpha)

    print(f"{svg_path} -> {out_path}  ({w}x{h}, {len(alpha)+4} bytes)")


if __name__ == "__main__":
    main()
