/*
 * atl.c — Minimal software renderer for a font-based UI
 *
 * Renders a grid of ASCII characters directly to the Linux framebuffer
 * (/dev/fb0).  Each character is sourced from the public-domain 8×8 bitmap
 * font in font8x8_basic.h, scaled up to the larger GLYPH_W×GLYPH_H cell
 * size at atlas-build time, and stored pre-rasterised so that every frame
 * is nothing but a grid of memcpy calls.
 *
 * Build:
 *   gcc -O3 -o atl atl.c
 * Run (needs framebuffer access, e.g. as root or in the 'video' group):
 *   ./atl
 */

#include "font8x8_basic.h"

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
 * Display / glyph geometry
 * ---------------------------------------------------------------------- */

#define WIN_W      800
#define WIN_H      480

#ifndef FONT_SIZE
#define FONT_SIZE 24
#endif

#define NUM_GLYPHS 128   /* one slot per ASCII code point (matches font8x8) */
#define GLYPH_W    FONT_SIZE   /* pixel width of one scaled glyph cell            */
#define GLYPH_H    FONT_SIZE   /* pixel height of one scaled glyph cell           */

/* Source font dimensions — fixed by font8x8_basic.h */
#define FONT_W     8
#define FONT_H     8

/* Number of glyph cells that fit in the window */
#define COLS  (WIN_W / GLYPH_W)
#define ROWS  (WIN_H / GLYPH_H)

/* -------------------------------------------------------------------------
 * Pixel / colour types
 *
 * Packed 32-bit ARGB: maps directly to the most common 32 bpp framebuffer
 * layout and lets the compiler move four bytes at a time.
 * ---------------------------------------------------------------------- */

typedef uint32_t pixel_t;   /* 0xAARRGGBB */

static inline pixel_t rgba(uint8_t r, uint8_t g, uint8_t b, uint8_t a)
{
    return ((uint32_t)a << 24) | ((uint32_t)r << 16)
         | ((uint32_t)g <<  8) | (uint32_t)b;
}

/* -------------------------------------------------------------------------
 * Glyph type
 *
 * A glyph is a flat GLYPH_W×GLYPH_H pixel array, pre-rasterised at startup.
 * The atlas holds one entry per ASCII code point so blitting is memcpy-only.
 * ---------------------------------------------------------------------- */

typedef struct {
    pixel_t px[GLYPH_H][GLYPH_W];
} glyph;

static glyph atlas[NUM_GLYPHS];

/* -------------------------------------------------------------------------
 * Colour palette
 *
 * Vivid colours assigned randomly to each atlas entry so the grid is
 * visually varied.  The background behind each character is transparent
 * (alpha=0) so the dark window fill shows through.
 * ---------------------------------------------------------------------- */

static const pixel_t palette[] = {
    0xFFE63946,   /* vivid red      */
    0xFFF4A261,   /* warm orange    */
    0xFF2A9D8F,   /* teal           */
    0xFFE9C46A,   /* golden yellow  */
    0xFFA8DADC,   /* light sky-blue */
    0xFF6A4C93,   /* purple         */
    0xFF52B788,   /* mint green     */
    0xFFFF6B6B,   /* coral          */
};
#define PALETTE_LEN ((int)(sizeof palette / sizeof *palette))

static pixel_t select_random_color(void)
{
    return palette[rand() % PALETTE_LEN];
}

/* -------------------------------------------------------------------------
 * Font scaling
 *
 * The source font is 8×8 pixels.  We scale it up to GLYPH_W×GLYPH_H using
 * nearest-neighbour (point) sampling: for each destination pixel (dx, dy)
 * we map back to source pixel (sx, sy) and test the corresponding bit.
 *
 * Source bit extraction:
 *   Each row is one byte.  Bit n (0=leftmost) is (byte >> n) & 1.
 *
 * This runs once at startup so its cost is irrelevant; clarity is preferred
 * over micro-optimisation here.
 * ---------------------------------------------------------------------- */

static void render_font_glyph(glyph *g, int ascii, pixel_t color)
{
    const unsigned char *rows = (const unsigned char *)font8x8_basic[ascii];

    for (int dy = 0; dy < GLYPH_H; dy++) {
        /* Map destination row back to source row (0–7) */
        int sy = (dy * FONT_H) / GLYPH_H;

        for (int dx = 0; dx < GLYPH_W; dx++) {
            /* Map destination column back to source column (0–7) */
            int sx = (dx * FONT_W) / GLYPH_W;

            /* Extract the bit: LSB = leftmost pixel in the row */
            int lit = (rows[sy] >> sx) & 1;

            g->px[dy][dx] = lit ? color : 0x00000000;
        }
    }
}

/* -------------------------------------------------------------------------
 * Atlas initialisation
 *
 * Scales every ASCII character from font8x8_basic into its atlas slot,
 * each in a randomly chosen foreground colour.
 * ---------------------------------------------------------------------- */

static void draw_atlas(void)
{
    for (int i = 0; i < NUM_GLYPHS; i++) {
        pixel_t color = select_random_color();
        render_font_glyph(&atlas[i], i, color);
    }
}

/* -------------------------------------------------------------------------
 * Character sequencer
 *
 * Returns successive ASCII indices (wrapping at NUM_GLYPHS) so the render
 * loop cycles through every character in order.
 * ---------------------------------------------------------------------- */

static unsigned char get_char(void)
{
   return (unsigned char)(32 + rand() % (126 - 32 + 1));
}

/* -------------------------------------------------------------------------
 * Framebuffer helpers
 * ---------------------------------------------------------------------- */

typedef struct {
    int       fd;
    pixel_t  *mem;
    size_t    size;
    int       width;
    int       height;
    int       stride;   /* pixels per scan-line; may exceed width */
} framebuf;

static int fb_open(framebuf *fb, const char *path)
{
    struct fb_var_screeninfo vinfo;
    struct fb_fix_screeninfo finfo;

    fb->fd = open(path, O_RDWR);
    if (fb->fd < 0) { perror("open"); return -1; }

    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("ioctl VSCREENINFO"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("ioctl FSCREENINFO"); return -1; }

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

static void fb_fill_rect(framebuf *fb, int x, int y, int w, int h, pixel_t color)
{
    for (int row = y; row < y + h && row < fb->height; row++) {
        pixel_t *line = fb->mem + row * fb->stride;
        for (int col = x; col < x + w && col < fb->width; col++)
            line[col] = color;
    }
}

/*
 * fb_blit_glyph — copies one pre-rasterised glyph into the framebuffer at
 * pixel position (dx, dy).  Each row is a single memcpy; no per-pixel logic
 * runs at blit time.
 */
static void fb_blit_glyph(framebuf *fb, int dx, int dy, const glyph *g)
{
    for (int gy = 0; gy < GLYPH_H; gy++) {
        int screen_y = dy + gy;
        if (screen_y < 0 || screen_y >= fb->height) continue;
        pixel_t *dst = fb->mem + screen_y * fb->stride + dx;
        memcpy(dst, g->px[gy], GLYPH_W * sizeof(pixel_t));
    }
}

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(void)
{
    srand((unsigned)time(NULL));

    /* Scale all 128 font glyphs into the atlas, each in a random colour */
    draw_atlas();

    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) {
        fprintf(stderr, "Could not open framebuffer.\n");
        return 1;
    }

    /* Dark blue background visible through the transparent glyph cells */
    const pixel_t bg = rgba(10, 15, 40, 255);
    fb_fill_rect(&fb, 0, 0, WIN_W, WIN_H, bg);

    /*
     * Main render loop — tiles the window with successive atlas glyphs.
     * All per-pixel work was done at atlas-build time; this loop is pure
     * memcpy and therefore limited only by memory bandwidth.
     */
    while (1) {
        for (int iy = 0; iy < ROWS; iy++) {
            for (int ix = 0; ix < COLS; ix++) {
                int x = ix * GLYPH_W;
                int y = iy * GLYPH_H;
                fb_blit_glyph(&fb, x, y, &atlas[get_char()]);
            }
        }
        /* Coarse ~30 fps throttle; variable frame rate is fine */
        usleep(33333);
    }

    fb_close(&fb);
    return 0;
}
