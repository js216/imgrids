/*
 * atl.c — Minimal software renderer, proportional font edition
 *
 * All per-frame work is a single memcpy from a back-buffer to the
 * framebuffer — one call per scanline row of WIN_H rows.  The back-buffer
 * is rebuilt each frame by stamping pre-composited glyph bitmaps into it;
 * because it is a plain malloc'd region (not mmap'd I/O memory) the CPU
 * cache behaves well and throughput stays low.
 *
 * Build:
 *   python3 font_to_c.py Roboto.ttf 12 > Roboto.h
 *   gcc -O3 -o atl atl.c
 */

#include "Roboto.h"
#define FONT_TABLE   Roboto
#define FONT_CELL_H  ROBOTO_CELL_H
typedef Roboto_glyph_t font_glyph_t;

#include <fcntl.h>
#include <linux/fb.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

/* -------------------------------------------------------------------------
 * Geometry / background colour
 * ---------------------------------------------------------------------- */

#define WIN_W  800
#define WIN_H  480

/* Background — must match BG_* used during cache compositing */
#define BG_R  10
#define BG_G  15
#define BG_B  40

/* -------------------------------------------------------------------------
 * Pixel type
 * ---------------------------------------------------------------------- */

typedef uint32_t pixel_t;

static inline pixel_t make_rgb(uint8_t r, uint8_t g, uint8_t b)
{
    return 0xFF000000u | ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
}

static const pixel_t BG_PIXEL = 0xFF000000u
    | ((uint32_t)BG_R << 16) | ((uint32_t)BG_G << 8) | BG_B;

/* -------------------------------------------------------------------------
 * Colour palette
 * ---------------------------------------------------------------------- */

static const pixel_t palette[] = {
    0xFFE63946, 0xFFF4A261, 0xFF2A9D8F, 0xFFE9C46A,
    0xFFA8DADC, 0xFF6A4C93, 0xFF52B788, 0xFFFF6B6B,
};
#define PALETTE_LEN ((int)(sizeof palette / sizeof *palette))

static pixel_t random_color(void) { return palette[rand() % PALETTE_LEN]; }

/* -------------------------------------------------------------------------
 * Glyph cache — pre-composited against the background
 *
 * Each pixel is already the final opaque RGB value; blitting is memcpy only.
 * ---------------------------------------------------------------------- */

typedef struct {
    int      advance_w;
    pixel_t *px;          /* advance_w × FONT_CELL_H, row-major */
} cached_glyph;

static cached_glyph cache[128];

static void build_cache(void)
{
    for (int code = 0; code < 128; code++) {
        const font_glyph_t *src = &FONT_TABLE[code];
        cache[code].advance_w = src->advance_w;

        if (src->advance_w <= 0 || !src->mask) { cache[code].px = NULL; continue; }

        int n = src->advance_w * FONT_CELL_H;
        cache[code].px = malloc((size_t)n * sizeof(pixel_t));
        if (!cache[code].px) { perror("malloc"); exit(1); }

        pixel_t  color = random_color();
        uint8_t  fg_r  = (color >> 16) & 0xFF;
        uint8_t  fg_g  = (color >>  8) & 0xFF;
        uint8_t  fg_b  =  color        & 0xFF;

        for (int i = 0; i < n; i++) {
            uint32_t a   = src->mask[i];
            uint32_t inv = 255 - a;
            uint8_t  r   = (uint8_t)((fg_r * a + BG_R * inv) / 255);
            uint8_t  g   = (uint8_t)((fg_g * a + BG_G * inv) / 255);
            uint8_t  b   = (uint8_t)((fg_b * a + BG_B * inv) / 255);
            cache[code].px[i] = 0xFF000000u
                               | ((uint32_t)r << 16)
                               | ((uint32_t)g <<  8)
                               | b;
        }
    }
}

/* -------------------------------------------------------------------------
 * Back-buffer
 *
 * WIN_W × WIN_H pixels of ordinary heap memory.  We compose the frame here
 * first, then push it to the framebuffer in one pass.  Writing to cached
 * heap memory is much faster than writing to mmap'd I/O memory pixel-by-
 * pixel, so total CPU time drops even though we touch each pixel twice.
 * ---------------------------------------------------------------------- */

static pixel_t backbuf[WIN_H][WIN_W];

static void backbuf_fill_bg(void)
{
    for (int y = 0; y < WIN_H; y++)
        for (int x = 0; x < WIN_W; x++)
            backbuf[y][x] = BG_PIXEL;
}

/* Stamp one cached glyph into the back-buffer at (dx, dy) */
static void backbuf_stamp(int dx, int dy, const cached_glyph *g)
{
    if (!g->px || g->advance_w <= 0) return;
    for (int gy = 0; gy < FONT_CELL_H; gy++) {
        int sy = dy + gy;
        if (sy < 0 || sy >= WIN_H) continue;
        const pixel_t *src = g->px + gy * g->advance_w;
        /* clamp width at right edge */
        int w = g->advance_w;
        if (dx + w > WIN_W) w = WIN_W - dx;
        if (w <= 0) continue;
        memcpy(&backbuf[sy][dx], src, (size_t)w * sizeof(pixel_t));
    }
}

/* -------------------------------------------------------------------------
 * Framebuffer
 * ---------------------------------------------------------------------- */

typedef struct {
    int      fd;
    pixel_t *mem;
    size_t   size;
    int      width;
    int      height;
    int      stride;
} framebuf;

static int fb_open(framebuf *fb, const char *path)
{
    struct fb_var_screeninfo vinfo;
    struct fb_fix_screeninfo finfo;

    fb->fd = open(path, O_RDWR);
    if (fb->fd < 0) { perror("open"); return -1; }
    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("VSCREENINFO"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("FSCREENINFO"); return -1; }

    fb->width  = (int)vinfo.xres;
    fb->height = (int)vinfo.yres;
    fb->stride = (int)(finfo.line_length / (vinfo.bits_per_pixel / 8));
    fb->size   = finfo.smem_len;

    fb->mem = mmap(NULL, fb->size, PROT_READ | PROT_WRITE, MAP_SHARED, fb->fd, 0);
    if (fb->mem == MAP_FAILED) { perror("mmap"); return -1; }
    return 0;
}

static void fb_close(framebuf *fb)
{
    if (fb->mem && fb->mem != MAP_FAILED) munmap(fb->mem, fb->size);
    if (fb->fd >= 0) close(fb->fd);
}

/*
 * Push the back-buffer to the framebuffer — one memcpy per row.
 * If the fb stride equals WIN_W we could do this in a single call,
 * but row-by-row is safe regardless of stride.
 */
static void fb_flip(framebuf *fb)
{
    for (int y = 0; y < WIN_H && y < fb->height; y++) {
        pixel_t *dst = fb->mem + y * fb->stride;
        memcpy(dst, backbuf[y], WIN_W * sizeof(pixel_t));
    }
}

/* -------------------------------------------------------------------------
 * Character source — pure random, never stalls
 * ---------------------------------------------------------------------- */

static unsigned char get_char(void)
{
    return (unsigned char)(32 + rand() % (126 - 32 + 1));
}

/* -------------------------------------------------------------------------
 * Frame composition
 * ---------------------------------------------------------------------- */

static void render_frame(void)
{
    backbuf_fill_bg();

    int cursor_x = 0;
    int cursor_y = 0;

    while (cursor_y + FONT_CELL_H <= WIN_H) {
        unsigned char code = get_char();
        const cached_glyph *g = &cache[code];

        if (cursor_x + g->advance_w > WIN_W) {
            cursor_x  = 0;
            cursor_y += FONT_CELL_H;
            if (cursor_y + FONT_CELL_H > WIN_H) break;
        }

        backbuf_stamp(cursor_x, cursor_y, g);
        cursor_x += g->advance_w;
    }
}

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(void)
{
    srand((unsigned)time(NULL));
    build_cache();

    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) {
        fprintf(stderr, "Could not open framebuffer.\n");
        return 1;
    }

    while (1) {
        render_frame();
        fb_flip(&fb);
        usleep(33333);
    }

    fb_close(&fb);
    return 0;
}
