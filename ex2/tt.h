/*
 * tt.h — Proportional font renderer (.font files from font_to_bin.py).
 */

#ifndef TT_H
#define TT_H

#include <stdint.h>

typedef uint32_t pixel_t;

typedef struct tt_atlas tt_atlas;

/*
 * tt_init — load a .font file and composite every glyph against fg/bg.
 *   fg : foreground (text) colour — 0xAARRGGBB
 *   bg : background colour        — must match your framebuffer background
 */
tt_atlas *tt_init(const char *path, pixel_t fg, pixel_t bg);

/*
 * tt_draw — blit a NUL-terminated string into the framebuffer at (x, y).
 * Glyphs are variable-width; the pen advances by each glyph's natural advance_w.
 */
void tt_draw(const tt_atlas *atlas,
             pixel_t *fb, int fb_stride,
             int x, int y,
             const char *text);

int tt_cell_h(const tt_atlas *atlas);
int tt_text_w(const tt_atlas *atlas, const char *text);

#endif /* TT_H */
