/*
 * mono.c — framebuffer renderer, monospace grid, proportional .font file
 *
 * Usage:  ./mono <font.font>
 *
 * Reads a .font file produced by font_to_bin.py.  At load time each glyph
 * is padded to cell_w (the widest glyph) and composited against a random
 * fg colour + fixed bg.  Every atlas entry is then identical in size, so
 * the render loop is COLS×ROWS×cell_h memcpy calls with no branching.
 *
 * Build:  gcc -O3 -o mono mono.c
 */

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

#define WIN_W  800
#define WIN_H  480
#define BG_R   10
#define BG_G   15
#define BG_B   40

typedef uint32_t pixel_t;

static const pixel_t BG_PX = 0xFF000000u
    | ((uint32_t)BG_R << 16) | ((uint32_t)BG_G << 8) | BG_B;

static const pixel_t palette[] = {
    0xFFE63946, 0xFFF4A261, 0xFF2A9D8F, 0xFFE9C46A,
    0xFFA8DADC, 0xFF6A4C93, 0xFF52B788, 0xFFFF6B6B,
};
#define PALETTE_LEN ((int)(sizeof palette / sizeof *palette))
static pixel_t random_color(void) { return palette[rand() % PALETTE_LEN]; }

/* -------------------------------------------------------------------------
 * Atlas — all entries padded to cell_w, composited, ready for direct memcpy
 * ---------------------------------------------------------------------- */

static int      cell_w, cell_h, cols, rows;
static pixel_t *atlas[128];
static pixel_t *atlas_buf;

static int load_font(const char *path)
{
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return -1; }

    /* Header: cell_h (uint32), then 128 x advance_w (uint16) */
    uint32_t ch;
    if (fread(&ch, 4, 1, f) != 1) { perror("header"); return -1; }
    cell_h = (int)ch;

    uint16_t aw[128];
    if (fread(aw, 2, 128, f) != 128) { perror("widths"); return -1; }

    /* cell_w = widest glyph */
    cell_w = 1;
    for (int i = 0; i < 128; i++)
        if (aw[i] > cell_w) cell_w = aw[i];

    cols = WIN_W / cell_w;
    rows = WIN_H / cell_h;

    int cells = cell_w * cell_h;
    atlas_buf = malloc(128 * (size_t)cells * sizeof(pixel_t));
    if (!atlas_buf) { perror("malloc"); return -1; }

    uint8_t *mask = malloc((size_t)cell_w * cell_h);  /* temp, max glyph size */
    if (!mask) { perror("malloc mask"); return -1; }

    for (int code = 0; code < 128; code++) {
        atlas[code] = atlas_buf + code * cells;

        int gw = aw[code];   /* this glyph's natural width; 0 = unused slot */

        /* read the glyph's natural-width mask if it has one */
        if (gw > 0) {
            if (fread(mask, 1, (size_t)gw * cell_h, f) != (size_t)gw * cell_h) {
                perror("glyph data"); free(mask); return -1;
            }
        }

        pixel_t color = (code >= 32 && code <= 126) ? random_color() : BG_PX;
        uint8_t fg_r  = (color >> 16) & 0xFF;
        uint8_t fg_g  = (color >>  8) & 0xFF;
        uint8_t fg_b  =  color        & 0xFF;

        for (int row = 0; row < cell_h; row++) {
            for (int col = 0; col < cell_w; col++) {
                /* alpha from mask for glyph columns, 0 (bg) for padding */
                uint32_t a = (gw > 0 && col < gw) ? mask[row * gw + col] : 0;
                uint32_t inv = 255 - a;
                uint8_t r = (uint8_t)((fg_r * a + BG_R * inv) / 255);
                uint8_t g = (uint8_t)((fg_g * a + BG_G * inv) / 255);
                uint8_t b = (uint8_t)((fg_b * a + BG_B * inv) / 255);
                atlas[code][row * cell_w + col] = 0xFF000000u
                    | ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
            }
        }
    }

    free(mask);
    fclose(f);
    return 0;
}

/* -------------------------------------------------------------------------
 * Framebuffer
 * ---------------------------------------------------------------------- */

typedef struct { int fd; pixel_t *mem; size_t size; int stride; } framebuf;

static int fb_open(framebuf *fb, const char *path)
{
    struct fb_var_screeninfo vinfo;
    struct fb_fix_screeninfo finfo;
    fb->fd = open(path, O_RDWR);
    if (fb->fd < 0) { perror("open"); return -1; }
    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("vscreeninfo"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("fscreeninfo"); return -1; }
    fb->stride = (int)(finfo.line_length / (vinfo.bits_per_pixel / 8));
    fb->size   = finfo.smem_len;
    fb->mem    = mmap(NULL, fb->size, PROT_READ | PROT_WRITE, MAP_SHARED, fb->fd, 0);
    if (fb->mem == MAP_FAILED) { perror("mmap"); return -1; }
    return 0;
}

static void fb_close(framebuf *fb)
{
    if (fb->mem && fb->mem != MAP_FAILED) munmap(fb->mem, fb->size);
    if (fb->fd >= 0) close(fb->fd);
}

/* -------------------------------------------------------------------------
 * Render loop — no branching, no arithmetic, just memcpy
 * ---------------------------------------------------------------------- */

static void render_frame(framebuf *fb)
{
    for (int iy = 0; iy < rows; iy++) {
        for (int ix = 0; ix < cols; ix++) {
            unsigned char code = (unsigned char)(32 + rand() % (126 - 32 + 1));
            const pixel_t *src = atlas[code];
            int dx = ix * cell_w;
            int dy = iy * cell_h;
            for (int gy = 0; gy < cell_h; gy++) {
                pixel_t *dst = fb->mem + (dy + gy) * fb->stride + dx;
                memcpy(dst, src + gy * cell_w, (size_t)cell_w * sizeof(pixel_t));
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc != 2) { fprintf(stderr, "Usage: %s <font.font>\n", argv[0]); return 1; }
    srand((unsigned)time(NULL));
    if (load_font(argv[1]) < 0) return 1;
    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) return 1;
    while (1) { render_frame(&fb); usleep(33333); }
    fb_close(&fb);
    free(atlas_buf);
    return 0;
}
