/*
 * atl.c — Minimal software renderer for a shape-based UI
 *
 * Renders a grid of colorful glyphs (geometric shapes) directly to the Linux
 * framebuffer device (/dev/fb0).  Each glyph is a pre-rasterised ARGB bitmap
 * stored in an atlas array so that blitting is a straight memcpy with no
 * per-frame rasterisation overhead.
 *
 * Build:
 *   gcc -O2 -o atl atl.c
 * Run (needs framebuffer access, e.g. as root or in the 'video' group):
 *   ./atl
 */

#include <fcntl.h>
#include <linux/fb.h>
#include <math.h>
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

#define NUM_GLYPHS 128   /* one slot per printable-ish ASCII code point */
#define GLYPH_W    FONT_SIZE /* pixel width of one glyph cell                */
#define GLYPH_H    FONT_SIZE /* pixel height of one glyph cell               */

/* Number of glyph cells that fit in the window */
#define COLS  (WIN_W / GLYPH_W)   /* 13 */
#define ROWS  (WIN_H / GLYPH_H)   /*  6 */

/* -------------------------------------------------------------------------
 * Pixel / colour types
 *
 * We use packed 32-bit ARGB (alpha in the most-significant byte) because it
 * maps directly to the most common 32 bpp framebuffer layout and lets the
 * compiler blit four bytes at a time.
 * ---------------------------------------------------------------------- */

typedef uint32_t pixel_t;   /* 0xAARRGGBB */

/* Convenience constructor */
static inline pixel_t rgba(uint8_t r, uint8_t g, uint8_t b, uint8_t a)
{
    return ((uint32_t)a << 24) | ((uint32_t)r << 16)
         | ((uint32_t)g <<  8) | (uint32_t)b;
}

/* -------------------------------------------------------------------------
 * Glyph type
 *
 * A glyph is simply a flat array of pixels representing one GLYPH_W×GLYPH_H
 * cell.  Storing them pre-rasterised means rendering the full screen is
 * nothing but COLS×ROWS memcpy calls — as fast as we can get without GPU
 * involvement.
 * ---------------------------------------------------------------------- */

typedef struct {
    pixel_t px[GLYPH_H][GLYPH_W];
} glyph;

/* The global glyph atlas — one entry per ASCII slot */
static glyph atlas[NUM_GLYPHS];

/* -------------------------------------------------------------------------
 * Shape identifiers
 * ---------------------------------------------------------------------- */

typedef enum {
    SQUARE   = 0,
    CIRCLE   = 1,
    TRIANGLE = 2,
    NUM_SHAPES
} shape_t;

/* -------------------------------------------------------------------------
 * Palette helpers
 *
 * A small set of vivid colours; each shape gets one chosen at random so
 * the atlas stays visually varied.
 * ---------------------------------------------------------------------- */

static const pixel_t palette[] = {
    0xFFE63946,   /* vivid red      */
    0xFFF4A261,   /* warm orange    */
    0xFF2A9D8F,   /* teal           */
    0xFFE9C46A,   /* golden yellow  */
    0xFF264653,   /* dark slate     */
    0xFFA8DADC,   /* light sky-blue */
    0xFF6A4C93,   /* purple         */
    0xFF52B788,   /* mint green     */
};
#define PALETTE_LEN ((int)(sizeof palette / sizeof *palette))

static pixel_t select_random_color(void)
{
    return palette[rand() % PALETTE_LEN];
}

/* -------------------------------------------------------------------------
 * Glyph rasterisers
 *
 * Each function fills a glyph cell with a specific geometric shape drawn in
 * the supplied foreground colour against a transparent background (alpha=0).
 * All coordinates are in glyph-local space, origin at top-left.
 * ---------------------------------------------------------------------- */

/*
 * render_square — fills the interior of a rectangle that is inset slightly
 * from the cell edges so glyphs have visible spacing when tiled.
 */
static void render_square(glyph *g, pixel_t color)
{
    const int margin = 6;
    for (int y = 0; y < GLYPH_H; y++)
        for (int x = 0; x < GLYPH_W; x++)
            g->px[y][x] = (x >= margin && x < GLYPH_W - margin &&
                           y >= margin && y < GLYPH_H - margin)
                          ? color : 0x00000000;
}

/*
 * render_circle — rasterises a filled circle whose diameter spans most of
 * the cell.  Uses the standard (x-cx)²+(y-cy)²≤r² test per pixel.
 */
static void render_circle(glyph *g, pixel_t color)
{
    const float cx = GLYPH_W / 2.0f;
    const float cy = GLYPH_H / 2.0f;
    /* radius is just short of the smaller half-dimension */
    const float r  = (GLYPH_W < GLYPH_H ? GLYPH_W : GLYPH_H) / 2.0f - 5.0f;

    for (int y = 0; y < GLYPH_H; y++) {
        float dy = y - cy;
        for (int x = 0; x < GLYPH_W; x++) {
            float dx = x - cx;
            g->px[y][x] = (dx*dx + dy*dy <= r*r) ? color : 0x00000000;
        }
    }
}

/*
 * render_triangle — rasterises a filled upward-pointing equilateral triangle
 * using barycentric/edge-function tests so there are no float-precision gaps.
 */
static void render_triangle(glyph *g, pixel_t color)
{
    const int margin = 6;
    /* Apex at top-centre; base spans the bottom of the cell */
    const int ax = GLYPH_W / 2,    ay = margin;
    const int bx = margin,          by = GLYPH_H - margin;
    const int cx = GLYPH_W - margin, cy = GLYPH_H - margin;

    for (int y = 0; y < GLYPH_H; y++) {
        for (int x = 0; x < GLYPH_W; x++) {
            /* Sign of the cross product for each edge tells which side we're on */
            int d0 = (bx - ax)*(y - ay) - (by - ay)*(x - ax);
            int d1 = (cx - bx)*(y - by) - (cy - by)*(x - bx);
            int d2 = (ax - cx)*(y - cy) - (ay - cy)*(x - cx);
            int inside = (d0 >= 0 && d1 >= 0 && d2 >= 0)
                      || (d0 <= 0 && d1 <= 0 && d2 <= 0);
            g->px[y][x] = inside ? color : 0x00000000;
        }
    }
}

/*
 * render_glyph — dispatch to the appropriate rasteriser and write the result
 * directly into the atlas slot pointed to by g.
 */
static void render_glyph(glyph *g, shape_t shape, pixel_t color)
{
    /* Clear the cell to fully transparent black first */
    memset(g, 0, sizeof *g);

    switch (shape) {
    case SQUARE:   render_square(g, color);   break;
    case CIRCLE:   render_circle(g, color);   break;
    case TRIANGLE: render_triangle(g, color); break;
    default:       break;  /* leave transparent for unknown slots */
    }
}

/* -------------------------------------------------------------------------
 * Atlas initialisation
 *
 * Cycles through all three shape types, assigning a fresh random colour to
 * each glyph slot so consecutive glyphs are never the same colour.
 * ---------------------------------------------------------------------- */

static void draw_atlas(void)
{
    for (int i = 0; i < NUM_GLYPHS; i++) {
        shape_t shape = (shape_t)(i % NUM_SHAPES);
        pixel_t color = select_random_color();
        render_glyph(&atlas[i], shape, color);
    }
}

/* -------------------------------------------------------------------------
 * Character sequencer
 *
 * Returns successive ASCII codes (wrapping at NUM_GLYPHS) so each glyph
 * cell on screen is filled with the next entry in the atlas.
 * ---------------------------------------------------------------------- */

static char get_char(void)
{
   return (unsigned char)(32 + rand() % (126 - 32 + 1));
}

/* -------------------------------------------------------------------------
 * Framebuffer helpers
 * ---------------------------------------------------------------------- */

typedef struct {
    int       fd;        /* open file descriptor for /dev/fb0  */
    pixel_t  *mem;       /* mmap'd base pointer                */
    size_t    size;      /* byte length of the mmap region     */
    int       width;     /* actual screen width in pixels      */
    int       height;    /* actual screen height in pixels     */
    int       stride;    /* pixels per scan-line (may > width) */
} framebuf;

/*
 * fb_open — opens /dev/fb0, queries its geometry via ioctl, and mmap's the
 * entire video memory region for direct pixel access.
 */
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
    /* stride in pixels — line_length is in bytes, bits_per_pixel covers depth */
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
 * fb_fill_rect — fills a rectangle of the framebuffer with a solid colour.
 * Used to paint the dark-blue background before blitting glyphs.
 */
static void fb_fill_rect(framebuf *fb, int x, int y, int w, int h, pixel_t color)
{
    for (int row = y; row < y + h && row < fb->height; row++) {
        pixel_t *line = fb->mem + row * fb->stride;
        for (int col = x; col < x + w && col < fb->width; col++)
            line[col] = color;
    }
}

/*
 * fb_blit_glyph — copies one glyph cell into the framebuffer at pixel
 * position (dx, dy), performing a straight overwrite (no alpha blending)
 * for maximum throughput.  Each row is one memcpy call.
 */
static void fb_blit_glyph(framebuf *fb, int dx, int dy, const glyph *g)
{
    for (int gy = 0; gy < GLYPH_H; gy++) {
        int screen_y = dy + gy;
        if (screen_y < 0 || screen_y >= fb->height) continue;

        pixel_t *dst = fb->mem + screen_y * fb->stride + dx;
        /* memcpy one full row of the glyph — GLYPH_W × 4 bytes */
        memcpy(dst, g->px[gy], GLYPH_W * sizeof(pixel_t));
    }
}

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(void)
{
    srand((unsigned)time(NULL));

    /* Build the pre-rasterised glyph atlas once at startup */
    draw_atlas();

    /* Open the Linux framebuffer */
    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) {
        fprintf(stderr, "Could not open framebuffer.\n");
        return 1;
    }

    /* Paint a dark blue background across the full window area */
    const pixel_t bg = rgba(10, 15, 40, 255);
    fb_fill_rect(&fb, 0, 0, WIN_W, WIN_H, bg);

    /*
     * Main render loop — tiles the full COLS×ROWS grid with atlas glyphs.
     * Because the atlas is static and blitting is just memcpy, this loop
     * runs as fast as the memory bandwidth allows with no recomputation.
     */
    while (1) {
        for (int iy = 0; iy < ROWS; iy++) {
            for (int ix = 0; ix < COLS; ix++) {
                int x = ix * GLYPH_W;
                int y = iy * GLYPH_H;
                unsigned char idx = (unsigned char)get_char() % NUM_GLYPHS;
                fb_blit_glyph(&fb, x, y, &atlas[idx]);
            }
        }
        /* Sleep ~33 ms between frames — no attempt to hit exactly 30 fps,
         * just a coarse throttle for minimum CPU usage */
        usleep(33333);
    }

    fb_close(&fb);
    return 0;
}
