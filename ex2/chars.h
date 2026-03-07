/*
 * chars.h — Bitmap font atlas renderer.
 */

#ifndef CHARS_H
#define CHARS_H

#include <stdint.h>

typedef uint32_t pixel_t;

typedef struct chars_atlas chars_atlas;

/*
 * chars_init — rasterise a 128-glyph atlas at glyph_w × glyph_h pixels.
 *   fg : foreground (text) colour — 0xAARRGGBB
 *   bg : background colour        — 0x00000000 = transparent
 */
chars_atlas *chars_init(int glyph_w, int glyph_h, pixel_t fg, pixel_t bg);

/*
 * chars_draw — blit a NUL-terminated string into the framebuffer at (x, y).
 */
void chars_draw(const chars_atlas *atlas,
                pixel_t *fb, int fb_stride,
                int x, int y,
                const char *text);

int chars_glyph_w(const chars_atlas *atlas);
int chars_glyph_h(const chars_atlas *atlas);

#endif /* CHARS_H */
