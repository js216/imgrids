/*
 * mono.c — Minimal framebuffer renderer, monospace grid
 *
 * Usage:  ./mono <font.font>
 *
 * The .font file is produced by font_to_bin.py:
 *   python3 font_to_bin.py Roboto.ttf 12 > Roboto.font
 *
 * At startup the font is loaded, each glyph composited against a random
 * foreground colour and the fixed background, and stored in a flat atlas.
 * The main loop is nothing but COLS×ROWS memcpy calls — one per cell per
 * frame.  No branching, no arithmetic, no back-buffer, no full-screen clear.
 *
 * Build:
 *   gcc -O3 -o mono mono.c
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

/* -------------------------------------------------------------------------
 * Display geometry / background colour
 * ---------------------------------------------------------------------- */

#define WIN_W  800
#define WIN_H  480

#define BG_R   10
#define BG_G   15
#define BG_B   40

/* -------------------------------------------------------------------------
 * Pixel type — packed 32-bit XRGB (0xFFRRGGBB)
 * ---------------------------------------------------------------------- */

typedef uint32_t pixel_t;

/* -------------------------------------------------------------------------
 * Font / atlas
 *
 * cell_w, cell_h: dimensions of every glyph cell (same for all 128 slots)
 * atlas[c]:       pointer into a flat buffer; CELL_W×CELL_H fully-opaque
 *                 pixels, pre-composited fg+bg — ready to memcpy directly
 *                 into the framebuffer row by row.
 * ---------------------------------------------------------------------- */

static int      cell_w, cell_h;
static int      cols,   rows;        /* cells that fit in WIN_W × WIN_H  */
static pixel_t *atlas[128];          /* one pre-composited bitmap per slot */
static pixel_t *atlas_buf = NULL;    /* backing allocation for all bitmaps */

static const pixel_t palette[] = {
    0xFFE63946, 0xFFF4A261, 0xFF2A9D8F, 0xFFE9C46A,
    0xFFA8DADC, 0xFF6A4C93, 0xFF52B788, 0xFFFF6B6B,
};
#define PALETTE_LEN ((int)(sizeof palette / sizeof *palette))
static pixel_t random_color(void) { return palette[rand() % PALETTE_LEN]; }

/*
 * load_font — read the binary .font file, composite each alpha mask against
 * a random foreground colour, store the result in atlas[].
 *
 * The compositing formula is the standard "over" with a constant background:
 *   out = fg * a/255 + bg * (1 - a/255)
 * Done once here; never repeated in the render loop.
 */
static int load_font(const char *path)
{
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return -1; }

    uint32_t hdr[2];
    if (fread(hdr, sizeof hdr, 1, f) != 1) { perror("fread header"); fclose(f); return -1; }
    cell_w = (int)hdr[0];
    cell_h = (int)hdr[1];
    cols   = WIN_W / cell_w;
    rows   = WIN_H / cell_h;

    int cells    = cell_w * cell_h;
    uint8_t *mask = malloc((size_t)cells);
    if (!mask) { perror("malloc mask"); fclose(f); return -1; }

    /* Single allocation for all 128 composited bitmaps */
    atlas_buf = malloc(128 * (size_t)cells * sizeof(pixel_t));
    if (!atlas_buf) { perror("malloc atlas"); free(mask); fclose(f); return -1; }

    const pixel_t bg_px = 0xFF000000u
                        | ((uint32_t)BG_R << 16)
                        | ((uint32_t)BG_G <<  8)
                        | BG_B;

    for (int code = 0; code < 128; code++) {
        atlas[code] = atlas_buf + code * cells;

        if (fread(mask, 1, (size_t)cells, f) != (size_t)cells) {
            perror("fread glyph"); free(mask); fclose(f); return -1;
        }

        pixel_t color = (code >= 32 && code <= 126) ? random_color() : bg_px;
        uint8_t fg_r  = (color >> 16) & 0xFF;
        uint8_t fg_g  = (color >>  8) & 0xFF;
        uint8_t fg_b  =  color        & 0xFF;

        for (int i = 0; i < cells; i++) {
            uint32_t a   = mask[i];
            uint32_t inv = 255 - a;
            uint8_t  r   = (uint8_t)((fg_r * a + BG_R * inv) / 255);
            uint8_t  g   = (uint8_t)((fg_g * a + BG_G * inv) / 255);
            uint8_t  b   = (uint8_t)((fg_b * a + BG_B * inv) / 255);
            atlas[code][i] = 0xFF000000u
                           | ((uint32_t)r << 16)
                           | ((uint32_t)g <<  8)
                           | b;
        }
    }

    free(mask);
    fclose(f);
    return 0;
}

/* -------------------------------------------------------------------------
 * Framebuffer
 * ---------------------------------------------------------------------- */

typedef struct {
    int      fd;
    pixel_t *mem;
    size_t   size;
    int      stride;   /* pixels per scanline */
} framebuf;

static int fb_open(framebuf *fb, const char *path)
{
    struct fb_var_screeninfo vinfo;
    struct fb_fix_screeninfo finfo;

    fb->fd = open(path, O_RDWR);
    if (fb->fd < 0) { perror("open"); return -1; }
    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("VSCREENINFO"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("FSCREENINFO"); return -1; }

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
 * Render loop
 *
 * For each cell: pick a random glyph, copy its rows directly into the
 * framebuffer.  That's it.  No clear, no back-buffer, no blending.
 * Each memcpy is cell_w*4 bytes — small enough to stay in L1 cache.
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

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "Usage: %s <font.font>\n", argv[0]);
        return 1;
    }

    srand((unsigned)time(NULL));

    if (load_font(argv[1]) < 0) return 1;

    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) return 1;

    while (1) {
        render_frame(&fb);
        usleep(33333);
    }

    fb_close(&fb);
    free(atlas_buf);
    return 0;
}
