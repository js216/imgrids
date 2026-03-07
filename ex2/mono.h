/*
 * mono.h — Monospace font renderer (.font files from font_to_bin.py).
 */

#ifndef MONO_H
#define MONO_H

#include <stdint.h>

typedef uint32_t pixel_t;

typedef struct mono_atlas mono_atlas;

/*
 * mono_init — load a .font file, pad all glyphs to the widest advance width,
 * and composite every glyph against fg/bg.
 *   fg : foreground (text) colour — 0xAARRGGBB
 *   bg : background colour        — must match your framebuffer background
 */
mono_atlas *mono_init(const char *path, pixel_t fg, pixel_t bg);

/*
 * mono_draw — blit a NUL-terminated string into the framebuffer at (x, y).
 * All glyphs have the same cell_w so the pen advances by cell_w per character.
 */
void mono_draw(const mono_atlas *atlas,
               pixel_t *fb, int fb_stride,
               int x, int y,
               const char *text);

int mono_cell_w(const mono_atlas *atlas);
int mono_cell_h(const mono_atlas *atlas);

#endif /* MONO_H */
