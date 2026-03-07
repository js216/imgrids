/*
 * tt.c — framebuffer renderer, proportional font, direct blit
 *
 * Usage:  ./tt <font.font>
 *
 * Reads a .font file produced by font_to_bin.py.  Each glyph is stored at
 * its natural advance_w; the atlas holds variable-width pre-composited
 * bitmaps.  The render loop writes each glyph's rows directly to the
 * framebuffer with no back-buffer, no clear, no blending.  Horizontal bleed
 * from wider previous characters is accepted — it will be overwritten within
 * a frame or two.  Vertical extent is always exactly cell_h, so there is no
 * vertical bleed.
 *
 * Build:  gcc -O3 -o tt tt.c
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
 * Atlas — variable-width entries, each advance_w[code] * cell_h pixels
 * ---------------------------------------------------------------------- */

static int      cell_h;
static int      adv[128];    /* advance_w per slot; 0 = unused           */
static pixel_t *atlas[128];  /* pre-composited bitmap, or NULL           */
static pixel_t *atlas_buf;   /* single allocation backing all bitmaps    */

static int load_font(const char *path)
{
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return -1; }

    uint32_t ch;
    if (fread(&ch, 4, 1, f) != 1) { perror("header"); return -1; }
    cell_h = (int)ch;

    uint16_t aw[128];
    if (fread(aw, 2, 128, f) != 128) { perror("widths"); return -1; }

    /* Total pixels across all glyphs for one allocation */
    size_t total = 0;
    for (int i = 0; i < 128; i++) total += aw[i] * cell_h;

    atlas_buf = malloc(total * sizeof(pixel_t));
    if (!atlas_buf) { perror("malloc atlas"); return -1; }

    pixel_t *ptr = atlas_buf;
    for (int code = 0; code < 128; code++) {
        int gw = aw[code];
        adv[code] = gw;

        if (gw == 0) { atlas[code] = NULL; continue; }

        atlas[code] = ptr;
        ptr += gw * cell_h;

        int n = gw * cell_h;
        uint8_t *mask = malloc((size_t)n);
        if (!mask) { perror("malloc mask"); return -1; }
        if (fread(mask, 1, (size_t)n, f) != (size_t)n) { perror("glyph"); free(mask); return -1; }

        pixel_t color = (code >= 32 && code <= 126) ? random_color() : BG_PX;
        uint8_t fg_r  = (color >> 16) & 0xFF;
        uint8_t fg_g  = (color >>  8) & 0xFF;
        uint8_t fg_b  =  color        & 0xFF;

        for (int i = 0; i < n; i++) {
            uint32_t a = mask[i], inv = 255 - a;
            uint8_t r = (uint8_t)((fg_r * a + BG_R * inv) / 255);
            uint8_t g = (uint8_t)((fg_g * a + BG_G * inv) / 255);
            uint8_t b = (uint8_t)((fg_b * a + BG_B * inv) / 255);
            atlas[code][i] = 0xFF000000u
                | ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
        }
        free(mask);
    }

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
 * Render loop
 *
 * Walks a cursor across WIN_W × WIN_H, wrapping at the right edge.
 * Each glyph is blitted row-by-row directly into the framebuffer.
 * No clear, no back-buffer, no blending — just memcpy.
 * ---------------------------------------------------------------------- */

static void render_frame(framebuf *fb)
{
    int cx = 0, cy = 0;

    while (cy + cell_h <= WIN_H) {
        unsigned char code = (unsigned char)(32 + rand() % (126 - 32 + 1));
        int gw = adv[code];
        if (gw == 0) continue;

        /* Wrap to next row if this glyph won't fit */
        if (cx + gw > WIN_W) {
            cx  = 0;
            cy += cell_h;
            if (cy + cell_h > WIN_H) break;
        }

        const pixel_t *src = atlas[code];
        for (int gy = 0; gy < cell_h; gy++) {
            pixel_t *dst = fb->mem + (cy + gy) * fb->stride + cx;
            memcpy(dst, src + gy * gw, (size_t)gw * sizeof(pixel_t));
        }
        cx += gw;
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
