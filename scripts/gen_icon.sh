#!/bin/bash
# Render all SVG icons in imgrids/icons/ to alpha masks at multiple sizes
cd /home/jk/projects/SR835_firmware/imgrids
for svg in icons/*.svg; do
    name=$(basename "$svg" .svg)
    for size in 16 24 32 48; do
        python3 scripts/render_icon.py "$svg" "$size" "icons/${name}_${size}.alpha"
    done
done
