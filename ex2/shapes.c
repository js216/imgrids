/*
 * shapes.c — Geometric shapes renderer.
 *
 * Rasterises squares, circles, and triangles into a 128-slot atlas at init
 * time using the supplied fg/bg colours.  shapes_draw is pure memcpy.
 */

#include "shapes.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#define NUM_GLYPHS 128

struct shapes_atlas {
    int      cell_size;
    pixel_t *glyphs;   /* flat: [NUM_GLYPHS][cell_size][cell_size] */
};

static inline pixel_t *glyph_ptr(const shapes_atlas *a, int i)
{
    return a->glyphs + (size_t)i * a->cell_size * a->cell_size;
}

/* -------------------------------------------------------------------------
 * Shape rasterisers — run only at init time
 * ---------------------------------------------------------------------- */

typedef enum { SHAPE_SQUARE = 0, SHAPE_CIRCLE = 1, SHAPE_TRIANGLE = 2, NUM_SHAPES } shape_t;

static void rasterise_square(pixel_t *dst, int sz, pixel_t fg, pixel_t bg)
{
    const int m = sz / 4;
    for (int y = 0; y < sz; y++)
        for (int x = 0; x < sz; x++)
            dst[y * sz + x] = (x >= m && x < sz-m && y >= m && y < sz-m) ? fg : bg;
}

static void rasterise_circle(pixel_t *dst, int sz, pixel_t fg, pixel_t bg)
{
    float cx = sz / 2.0f, cy = sz / 2.0f;
    float r  = sz / 2.0f - sz / 8.0f;
    for (int y = 0; y < sz; y++) {
        float dy = y - cy;
        for (int x = 0; x < sz; x++) {
            float dx = x - cx;
            dst[y * sz + x] = (dx*dx + dy*dy <= r*r) ? fg : bg;
        }
    }
}

static void rasterise_triangle(pixel_t *dst, int sz, pixel_t fg, pixel_t bg)
{
    const int m  = sz / 4;
    const int ax = sz/2, ay = m;
    const int bx = m,    by = sz - m;
    const int cx = sz-m, cy = sz - m;

    for (int y = 0; y < sz; y++) {
        for (int x = 0; x < sz; x++) {
            int d0 = (bx-ax)*(y-ay) - (by-ay)*(x-ax);
            int d1 = (cx-bx)*(y-by) - (cy-by)*(x-bx);
            int d2 = (ax-cx)*(y-cy) - (ay-cy)*(x-cx);
            int inside = (d0>=0 && d1>=0 && d2>=0) || (d0<=0 && d1<=0 && d2<=0);
            dst[y * sz + x] = inside ? fg : bg;
        }
    }
}

static void rasterise_glyph(pixel_t *dst, int sz, int idx, pixel_t fg, pixel_t bg)
{
    switch ((shape_t)(idx % NUM_SHAPES)) {
        case SHAPE_SQUARE:   rasterise_square  (dst, sz, fg, bg); break;
        case SHAPE_CIRCLE:   rasterise_circle  (dst, sz, fg, bg); break;
        case SHAPE_TRIANGLE: rasterise_triangle(dst, sz, fg, bg); break;
        default: memset(dst, 0, (size_t)sz * sz * sizeof(pixel_t)); break;
    }
}

/* -------------------------------------------------------------------------
 * Public API
 * ---------------------------------------------------------------------- */

shapes_atlas *shapes_init(int cell_size, pixel_t fg, pixel_t bg)
{
    shapes_atlas *a = malloc(sizeof *a);
    if (!a) return NULL;

    a->cell_size = cell_size;
    a->glyphs    = malloc((size_t)NUM_GLYPHS * cell_size * cell_size * sizeof(pixel_t));
    if (!a->glyphs) { free(a); return NULL; }

    for (int i = 0; i < NUM_GLYPHS; i++)
        rasterise_glyph(glyph_ptr(a, i), cell_size, i, fg, bg);

    return a;
}

/* Hyper-optimised: colours baked in, one memcpy per glyph row */
void shapes_draw(const shapes_atlas *atlas,
                 pixel_t *fb, int fb_stride,
                 int x, int y,
                 const char *text)
{
    const int sz = atlas->cell_size;
    int cx = x;

    for (const char *p = text; *p; p++, cx += sz) {
        const pixel_t *src = glyph_ptr(atlas, (unsigned char)*p & 0x7F);
        pixel_t       *row = fb + y * fb_stride + cx;

        for (int gy = 0; gy < sz; gy++)
            memcpy(row + gy * fb_stride, src + gy * sz, (size_t)sz * sizeof(pixel_t));
    }
}

int shapes_cell_size(const shapes_atlas *atlas) { return atlas->cell_size; }
