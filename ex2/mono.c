/*
 * mono.c — Monospace font renderer.
 *
 * Reads a .font file produced by font_to_bin.py.  Every glyph is padded to
 * cell_w (the widest advance) and composited against the caller-supplied
 * fg/bg at load time so mono_draw is a pure memcpy grid.
 */

#include "mono.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct mono_atlas {
    int       cell_w;
    int       cell_h;
    pixel_t  *glyphs[128]; /* all cell_w * cell_h — pointers into buf */
    pixel_t  *buf;
};

mono_atlas *mono_init(const char *path, pixel_t fg, pixel_t bg)
{
    uint8_t fg_r = (fg >> 16) & 0xFF, fg_g = (fg >> 8) & 0xFF, fg_b = fg & 0xFF;
    uint8_t bg_r = (bg >> 16) & 0xFF, bg_g = (bg >> 8) & 0xFF, bg_b = bg & 0xFF;

    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return NULL; }

    uint32_t ch;
    if (fread(&ch, 4, 1, f) != 1) { perror("mono header"); fclose(f); return NULL; }

    uint16_t aw[128];
    if (fread(aw, 2, 128, f) != 128) { perror("mono widths"); fclose(f); return NULL; }

    int cell_w = 1;
    for (int i = 0; i < 128; i++)
        if (aw[i] > cell_w) cell_w = aw[i];

    int cell_h = (int)ch;
    int cells  = cell_w * cell_h;

    mono_atlas *a = malloc(sizeof *a);
    if (!a) { fclose(f); return NULL; }
    a->cell_w = cell_w;
    a->cell_h = cell_h;
    a->buf    = malloc(128 * (size_t)cells * sizeof(pixel_t));
    if (!a->buf) { free(a); fclose(f); return NULL; }

    uint8_t *mask = malloc((size_t)cell_w * cell_h);
    if (!mask) { free(a->buf); free(a); fclose(f); return NULL; }

    for (int code = 0; code < 128; code++) {
        a->glyphs[code] = a->buf + code * cells;
        int gw = aw[code];

        if (gw > 0) {
            if (fread(mask, 1, (size_t)gw * cell_h, f) != (size_t)gw * cell_h) {
                perror("mono glyph"); free(mask); free(a->buf); free(a); fclose(f); return NULL;
            }
        }

        /* Composite into padded cell; columns beyond gw get pure bg (alpha=0) */
        for (int row = 0; row < cell_h; row++) {
            for (int col = 0; col < cell_w; col++) {
                uint32_t alpha = (gw > 0 && col < gw) ? mask[row * gw + col] : 0;
                uint32_t inv   = 255 - alpha;
                uint8_t r = (uint8_t)((fg_r * alpha + bg_r * inv) / 255);
                uint8_t g = (uint8_t)((fg_g * alpha + bg_g * inv) / 255);
                uint8_t b = (uint8_t)((fg_b * alpha + bg_b * inv) / 255);
                a->glyphs[code][row * cell_w + col] = 0xFF000000u
                    | ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
            }
        }
    }

    free(mask);
    fclose(f);
    return a;
}

/* Hyper-optimised: uniform cell_w, colours baked in, one memcpy per row */
void mono_draw(const mono_atlas *atlas,
               pixel_t *fb, int fb_stride,
               int x, int y,
               const char *text)
{
    const int cw = atlas->cell_w;
    const int ch = atlas->cell_h;
    int cx = x;

    for (const char *p = text; *p; p++, cx += cw) {
        const pixel_t *src = atlas->glyphs[(unsigned char)*p & 0x7F];
        pixel_t       *row = fb + y * fb_stride + cx;

        for (int gy = 0; gy < ch; gy++)
            memcpy(row + gy * fb_stride, src + gy * cw, (size_t)cw * sizeof(pixel_t));
    }
}

int mono_cell_w(const mono_atlas *atlas) { return atlas->cell_w; }
int mono_cell_h(const mono_atlas *atlas) { return atlas->cell_h; }
