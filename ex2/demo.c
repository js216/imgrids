/*
 * demo.c — Combined two-column framebuffer demo
 *
 * No clearing — every renderer overwrites its own pixels each frame.
 */

#include "shapes.h"
#include "chars.h"
#include "mono.h"
#include "tt.h"

#include <fcntl.h>
#include <linux/fb.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#define RED     0xFFE63946
#define GREEN   0xFF52B788
#define PURPLE  0xFF6A4C93
#define BLUE    0xFF4895EF
#define VIOLET  0xFF9B5DE5
#define PINK    0xFFF15BB5
#define WHITE   0xFFFFFFFF
#define GRAY    0xFFAAAAAA
#define TEAL    0xFF0D3B38
#define BROWN   0xFF2C1A0E
#define OLIVE   0xFF1E2010
#define SLATE   0xFF0F1535

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
    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("VSCREENINFO"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("FSCREENINFO"); return -1; }
    fb->stride = (int)(finfo.line_length / (vinfo.bits_per_pixel / 8));
    fb->size   = finfo.smem_len;
    fb->mem    = mmap(NULL, fb->size, PROT_READ | PROT_WRITE, MAP_SHARED, fb->fd, 0);
    if (fb->mem == MAP_FAILED) { perror("mmap"); return -1; }
    return 0;
}

/* -------------------------------------------------------------------------
 * Random text
 * ---------------------------------------------------------------------- */

static void random_text(char *buf, const int len)
{
    for (int i = 0; i < len; i++)
        buf[i] = (char)(32 + rand() % (126 - 32 + 1));
    buf[len] = '\0';
}

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(void)
{
    srand((unsigned)time(NULL));

    /* Left column atlases */
    shapes_atlas *l_shapes = shapes_init(24,        RED,    TEAL);
    chars_atlas  *l_chars  = chars_init (24, 24,    GREEN,  BROWN);
    mono_atlas   *l_mono   = mono_init  ("RobotoMono-Regular-24.font", PURPLE, OLIVE);
    tt_atlas     *l_tt     = tt_init    ("Roboto-Regular-24.font",   BLUE,   SLATE);

    /* Right column atlases */
    shapes_atlas *r_shapes = shapes_init(24,        VIOLET, TEAL);
    chars_atlas  *r_chars  = chars_init (24, 24,    PINK,   BROWN);
    mono_atlas   *r_mono   = mono_init  ("RobotoMono-Regular-12.font", WHITE,  OLIVE);
    tt_atlas     *r_tt     = tt_init    ("Roboto-Regular-12.font",   GRAY,   SLATE);

    if (!l_shapes || !l_chars || !l_mono || !l_tt ||
        !r_shapes || !r_chars || !r_mono || !r_tt) {
        fprintf(stderr, "init failed\n");
        return 1;
    }

    framebuf fb;
    if (fb_open(&fb, "/dev/fb0") < 0) return 1;

    /* Evenly space ROWS lines vertically */
    const int rows = 4;
    const int win_h = 480;
    const int y_step = win_h / (rows + 1);

    /* Row y positions, each centred on its step using the large cell height */
    const int y0 = y_step * 1 - shapes_cell_size(l_shapes) / 2;
    const int y1 = y_step * 2 - chars_glyph_h(l_chars)     / 2;
    const int y2 = y_step * 3 - mono_cell_h(l_mono)         / 2;
    const int y3 = y_step * 4 - tt_cell_h(l_tt)             / 2;

    /* Column positions */
    const int col_l = 20;      /* left column x */
    const int col_r = 420;      /* right column x */

    const int text_len = 10;

    while (1) {
        /* Fresh random text for each of the 8 slots */
        char t[8][text_len + 1];
        for (int i = 0; i < 8; i++)
            random_text(t[i], text_len);

        /* Left column */
        shapes_draw(l_shapes, fb.mem, fb.stride, col_l, y0, t[0]);
        chars_draw (l_chars,  fb.mem, fb.stride, col_l, y1, t[1]);
        mono_draw  (l_mono,   fb.mem, fb.stride, col_l, y2, t[2]);
        tt_draw    (l_tt,     fb.mem, fb.stride, col_l, y3, t[3]);

        /* Right column */
        shapes_draw(r_shapes, fb.mem, fb.stride, col_r, y0, t[4]);
        chars_draw (r_chars,  fb.mem, fb.stride, col_r, y1, t[5]);
        mono_draw  (r_mono,   fb.mem, fb.stride, col_r, y2, t[6]);
        tt_draw    (r_tt,     fb.mem, fb.stride, col_r, y3, t[7]);

        usleep(33333);
    }

    return 0;
}
