/*
 * tt.c — Proportional font renderer.
 *
 * Reads a .font file produced by font_to_bin.py.  Each glyph's alpha mask is
 * composited against the caller-supplied fg/bg at load time so tt_draw is
 * pure memcpy with no per-pixel work at draw time.
 */

#include "tt.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct tt_atlas {
    int       cell_h;
    int       adv[128];    /* advance_w per slot; 0 = unused        */
    pixel_t  *glyphs[128]; /* pre-composited bitmap, or NULL        */
    pixel_t  *buf;         /* single allocation backing all bitmaps */
};

tt_atlas *tt_init(const char *path, pixel_t fg, pixel_t bg)
{
    uint8_t fg_r = (fg >> 16) & 0xFF, fg_g = (fg >> 8) & 0xFF, fg_b = fg & 0xFF;
    uint8_t bg_r = (bg >> 16) & 0xFF, bg_g = (bg >> 8) & 0xFF, bg_b = bg & 0xFF;

    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return NULL; }

    uint32_t ch;
    if (fread(&ch, 4, 1, f) != 1) { perror("tt header"); fclose(f); return NULL; }

    uint16_t aw[128];
    if (fread(aw, 2, 128, f) != 128) { perror("tt widths"); fclose(f); return NULL; }

    size_t total = 0;
    for (int i = 0; i < 128; i++) total += (size_t)aw[i] * ch;

    tt_atlas *a = malloc(sizeof *a);
    if (!a) { fclose(f); return NULL; }
    a->cell_h = (int)ch;
    a->buf    = malloc(total * sizeof(pixel_t));
    if (!a->buf) { free(a); fclose(f); return NULL; }

    pixel_t *ptr = a->buf;
    for (int code = 0; code < 128; code++) {
        int gw = aw[code];
        a->adv[code] = gw;
        if (gw == 0) { a->glyphs[code] = NULL; continue; }

        a->glyphs[code] = ptr;
        ptr += gw * a->cell_h;

        int n = gw * a->cell_h;
        uint8_t *mask = malloc((size_t)n);
        if (!mask) { free(a->buf); free(a); fclose(f); return NULL; }
        if (fread(mask, 1, (size_t)n, f) != (size_t)n) {
            perror("tt glyph"); free(mask); free(a->buf); free(a); fclose(f); return NULL;
        }

        for (int i = 0; i < n; i++) {
            uint32_t alpha = mask[i], inv = 255 - alpha;
            uint8_t r = (uint8_t)((fg_r * alpha + bg_r * inv) / 255);
            uint8_t g = (uint8_t)((fg_g * alpha + bg_g * inv) / 255);
            uint8_t b = (uint8_t)((fg_b * alpha + bg_b * inv) / 255);
            a->glyphs[code][i] = 0xFF000000u | ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
        }
        free(mask);
    }

    fclose(f);
    return a;
}

/* Hyper-optimised: colours are baked in, inner loop is one memcpy per row */
void tt_draw(const tt_atlas *atlas,
             pixel_t *fb, int fb_stride,
             int x, int y,
             const char *text)
{
    int cx = x;
    for (const char *p = text; *p; p++) {
        int code = (unsigned char)*p & 0x7F;
        int gw   = atlas->adv[code];
        if (gw == 0 || !atlas->glyphs[code]) { cx += gw; continue; }

        const pixel_t *src = atlas->glyphs[code];
        pixel_t       *row = fb + y * fb_stride + cx;

        for (int gy = 0; gy < atlas->cell_h; gy++)
            memcpy(row + gy * fb_stride, src + gy * gw, (size_t)gw * sizeof(pixel_t));

        cx += gw;
    }
}

int tt_cell_h(const tt_atlas *atlas) { return atlas->cell_h; }

int tt_text_w(const tt_atlas *atlas, const char *text)
{
    int w = 0;
    for (const char *p = text; *p; p++)
        w += atlas->adv[(unsigned char)*p & 0x7F];
    return w;
}
