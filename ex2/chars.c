/*
 * chars.c — Bitmap font atlas renderer.
 *
 * Scales the public-domain 8×8 font (font8x8_basic.h) to any cell size using
 * nearest-neighbour sampling.  Colours are baked in at init time so
 * chars_draw is pure memcpy-equivalent with no per-pixel work.
 */

#include "chars.h"
#include "font8x8_basic.h"

#include <stdlib.h>
#include <string.h>

#define FONT_W     8
#define FONT_H     8
#define NUM_GLYPHS 128

struct chars_atlas {
    int      glyph_w;
    int      glyph_h;
    pixel_t *glyphs;   /* flat: [NUM_GLYPHS][glyph_h][glyph_w] */
};

static inline pixel_t *glyph_ptr(const chars_atlas *a, int i)
{
    return a->glyphs + (size_t)i * a->glyph_h * a->glyph_w;
}

/* Nearest-neighbour scale of one 8×8 glyph into dst — runs at init only */
static void rasterise_glyph(pixel_t *dst, int glyph_w, int glyph_h,
                             int ascii, pixel_t fg, pixel_t bg)
{
    const unsigned char *rows =
        (const unsigned char *)font8x8_basic[ascii & 0x7F];

    for (int dy = 0; dy < glyph_h; dy++) {
        int sy = (dy * FONT_H) / glyph_h;
        for (int dx = 0; dx < glyph_w; dx++) {
            int sx  = (dx * FONT_W) / glyph_w;
            int lit = (rows[sy] >> sx) & 1;
            dst[dy * glyph_w + dx] = lit ? fg : bg;
        }
    }
}

chars_atlas *chars_init(int glyph_w, int glyph_h, pixel_t fg, pixel_t bg)
{
    chars_atlas *a = malloc(sizeof *a);
    if (!a) return NULL;

    a->glyph_w = glyph_w;
    a->glyph_h = glyph_h;
    a->glyphs  = malloc((size_t)NUM_GLYPHS * glyph_h * glyph_w * sizeof(pixel_t));
    if (!a->glyphs) { free(a); return NULL; }

    for (int i = 0; i < NUM_GLYPHS; i++)
        rasterise_glyph(glyph_ptr(a, i), glyph_w, glyph_h, i, fg, bg);

    return a;
}

/* Hyper-optimised: colours are baked in, so the inner loop is pure memcpy */
void chars_draw(const chars_atlas *atlas,
                pixel_t *fb, int fb_stride,
                int x, int y,
                const char *text)
{
    const int gw = atlas->glyph_w;
    const int gh = atlas->glyph_h;
    int cx = x;

    for (const char *p = text; *p; p++, cx += gw) {
        const pixel_t *src = glyph_ptr(atlas, (unsigned char)*p & 0x7F);
        pixel_t       *row = fb + y * fb_stride + cx;

        for (int gy = 0; gy < gh; gy++)
            memcpy(row + gy * fb_stride, src + gy * gw, (size_t)gw * sizeof(pixel_t));
    }
}

int chars_glyph_w(const chars_atlas *atlas) { return atlas->glyph_w; }
int chars_glyph_h(const chars_atlas *atlas) { return atlas->glyph_h; }
