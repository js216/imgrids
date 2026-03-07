/*
 * demo.c — Framebuffer demo with a zero-overhead recursive grid layout DSL.
 *
 * C99 + POSIX only.
 *
 * Geometry defines:
 *
 *   WIN_X, WIN_Y     — top-left corner of the window within the framebuffer
 *   SCREEN_W         — window width  in pixels
 *   SCREEN_H         — window height in pixels
 *   MARGIN_X         — horizontal gap between border and grid
 *   MARGIN_Y         — vertical   gap between border and grid
 *
 * DSL — arbitrary nesting of ROW, COL, CELL:
 *
 *   node *layout =
 *       COL(1,
 *           ROW(1, CELL(API_TT, my_atlas, my_gen),
 *                  CELL(API_TT, my_atlas, other_gen) ),
 *           ROW(2, COL(1, CELL(API_SHAPES, l_shapes, gen_a),
 *                          CELL(API_MONO,   l_mono,   gen_b) ),
 *                  COL(1, CELL(API_TT,     r_tt,     gen_c) ) )
 *       );
 *
 * ROW(weight, ...) divides its box horizontally; children with higher weight
 * get proportionally more width.
 * COL(weight, ...) divides its box vertically by weight.
 * CELL(api, atlas, gen) is a leaf; gen is a `const char *(*)(void)` called
 * each frame to produce the text string.
 *
 * resolve_node() walks the tree once before the render loop and writes a
 * flat Cell[].  The tree is never touched again.
 *
 * Compile:
 *   cc -std=c99 -O2 -o demo demo.c shapes.o chars.o mono.o tt.o
 */

#include "shapes.h"
#include "chars.h"
#include "mono.h"
#include "tt.h"

#include <fcntl.h>
#include <linux/fb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

/* -------------------------------------------------------------------------
 * Geometry
 * ---------------------------------------------------------------------- */

#define WIN_X     0
#define WIN_Y     0
#define SCREEN_W  800
#define SCREEN_H  480
#define MARGIN_X  20
#define MARGIN_Y  20

/* -------------------------------------------------------------------------
 * Colours
 * ---------------------------------------------------------------------- */

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

typedef struct {
    int      fd;
    pixel_t *mem;
    size_t   size;
    int      stride;
    int      w;
    int      h;
} framebuf;

static int fb_open(framebuf *fb, const char *path)
{
    struct fb_var_screeninfo vinfo;
    struct fb_fix_screeninfo finfo;

    fb->fd = open(path, O_RDWR);
    if (fb->fd < 0) { perror("open"); return -1; }

    if (ioctl(fb->fd, FBIOGET_VSCREENINFO, &vinfo) < 0) { perror("VSCREENINFO"); return -1; }
    if (ioctl(fb->fd, FBIOGET_FSCREENINFO, &finfo) < 0) { perror("FSCREENINFO"); return -1; }

    fb->w      = (int)vinfo.xres;
    fb->h      = (int)vinfo.yres;
    fb->stride = (int)(finfo.line_length / (vinfo.bits_per_pixel / 8));
    fb->size   = finfo.smem_len;
    fb->mem    = mmap(NULL, fb->size, PROT_READ | PROT_WRITE, MAP_SHARED, fb->fd, 0);
    if (fb->mem == MAP_FAILED) { perror("mmap"); return -1; }
    return 0;
}

/* -------------------------------------------------------------------------
 * Border
 * ---------------------------------------------------------------------- */

static void draw_border(pixel_t *mem, int stride, int border_px)
{
    int x, y;

    for (y = WIN_Y; y < WIN_Y + border_px; y++)
        for (x = WIN_X; x < WIN_X + SCREEN_W; x++)
            mem[y * stride + x] = WHITE;

    for (y = WIN_Y + SCREEN_H - border_px; y < WIN_Y + SCREEN_H; y++)
        for (x = WIN_X; x < WIN_X + SCREEN_W; x++)
            mem[y * stride + x] = WHITE;

    for (y = WIN_Y; y < WIN_Y + SCREEN_H; y++) {
        for (x = WIN_X; x < WIN_X + border_px; x++)
            mem[y * stride + x] = WHITE;
        for (x = WIN_X + SCREEN_W - border_px; x < WIN_X + SCREEN_W; x++)
            mem[y * stride + x] = WHITE;
    }
}

/* -------------------------------------------------------------------------
 * Cell — the resolved functor
 * ---------------------------------------------------------------------- */

#define MAX_CELLS     256
#define MAX_TEXT_LEN   64   /* max characters any gen_fn will ever return  */

typedef enum { API_SHAPES, API_CHARS, API_MONO, API_TT } api_kind;
typedef const char *(*gen_fn)(void);

typedef struct {
    api_kind  api;
    void     *atlas;
    gen_fn    gen;
    int       x;
    int       y;
    char      last[MAX_TEXT_LEN + 1];  /* last text sent to the renderer   */
} Cell;

static void cell_draw(Cell *c, pixel_t *mem, int stride)
{
    const char *text = c->gen();
    if (strncmp(text, c->last, MAX_TEXT_LEN) == 0)
        return;
    strncpy(c->last, text, MAX_TEXT_LEN);
    c->last[MAX_TEXT_LEN] = '\0';

    switch (c->api) {
        case API_SHAPES: shapes_draw((shapes_atlas *)c->atlas, mem, stride, c->x, c->y, text); break;
        case API_CHARS:  chars_draw ((chars_atlas  *)c->atlas, mem, stride, c->x, c->y, text); break;
        case API_MONO:   mono_draw  ((mono_atlas   *)c->atlas, mem, stride, c->x, c->y, text); break;
        case API_TT:     tt_draw    ((tt_atlas     *)c->atlas, mem, stride, c->x, c->y, text); break;
    }
}

static int atlas_cell_h(api_kind api, void *atlas)
{
    switch (api) {
        case API_SHAPES: return shapes_cell_size((shapes_atlas *)atlas);
        case API_CHARS:  return chars_glyph_h   ((chars_atlas  *)atlas);
        case API_MONO:   return mono_cell_h     ((mono_atlas   *)atlas);
        case API_TT:     return tt_cell_h       ((tt_atlas     *)atlas);
    }
    return 0;
}

/* -------------------------------------------------------------------------
 * Layout DSL — recursive tagged-union node tree
 *
 * Each node is one of three variants:
 *
 *   NODE_CELL  — leaf; holds api, atlas, gen
 *   NODE_ROW   — splits bounding box horizontally among weighted children
 *   NODE_COL   — splits bounding box vertically   among weighted children
 *
 * The union ensures only the fields relevant to each variant are present.
 * weight applies to all variants; for the root node it is ignored.
 * ---------------------------------------------------------------------- */

typedef enum { NODE_CELL, NODE_ROW, NODE_COL } node_kind;

typedef struct node {
    node_kind kind;
    int       weight;
    union {
        struct {
            api_kind  api;
            void     *atlas;
            gen_fn    gen;
        } cell;
        struct {
            struct node **children;   /* NULL-terminated array of pointers */
        } split;
    } u;
} node;

#define CELL(a, atl, g)                                         \
    &(node){ NODE_CELL, 1,                                      \
        { .cell = { (a), (void *)(atl), (gen_fn)(g) } } }

#define ROW(w, ...)                                             \
    &(node){ NODE_ROW, (w),                                     \
        { .split = { (node *[]){ __VA_ARGS__, NULL } } } }

#define COL(w, ...)                                             \
    &(node){ NODE_COL, (w),                                     \
        { .split = { (node *[]){ __VA_ARGS__, NULL } } } }

/* -------------------------------------------------------------------------
 * resolve_node — recursive one-time layout pass
 *
 * Each node receives a bounding box (x, y, w, h).
 *
 *   NODE_ROW  — divides w proportionally by child weights; h unchanged
 *   NODE_COL  — divides h proportionally by child weights; w unchanged
 *   NODE_CELL — records (x, y) with the glyph vertically centred in box
 *
 * Returns the updated flat-array index after writing all leaves.
 * ---------------------------------------------------------------------- */

static int resolve_node(Cell *out, int idx, const node *n,
                        int x, int y, int w, int h)
{
    int i, n_children, total_weight;

    if (n->kind == NODE_CELL) {
        int ch       = atlas_cell_h(n->u.cell.api, n->u.cell.atlas);
        out[idx].api   = n->u.cell.api;
        out[idx].atlas = n->u.cell.atlas;
        out[idx].gen   = n->u.cell.gen;
        out[idx].x     = x;
        out[idx].y     = y + h / 2 - ch / 2;
        out[idx].last[0]  = '\0';
        return idx + 1;
    }

    /* Count children and sum weights */
    n_children   = 0;
    total_weight = 0;
    while (n->u.split.children[n_children] != NULL) {
        total_weight += n->u.split.children[n_children]->weight;
        n_children++;
    }

    /* Distribute space proportionally; track offset to avoid rounding drift */
    int offset = 0;
    for (i = 0; i < n_children; i++) {
        const node *child = n->u.split.children[i];
        int share = (i == n_children - 1)
            ? (n->kind == NODE_ROW ? w : h) - offset   /* last child gets remainder */
            : (n->kind == NODE_ROW ? w : h) * child->weight / total_weight;

        int cx, cy, cw, ch;
        if (n->kind == NODE_ROW) {
            cx = x + offset;  cy = y;
            cw = share;       ch = h;
            offset += share;
        } else {
            cx = x;           cy = y + offset;
            cw = w;           ch = share;
            offset += share;
        }
        idx = resolve_node(out, idx, child, cx, cy, cw, ch);
    }
    return idx;
}

/* -------------------------------------------------------------------------
 * Example text generators
 * ---------------------------------------------------------------------- */

static const char *gen_random(void)
{
    static char buf[32];
    int i;
    for (i = 0; i < 10; i++)
        buf[i] = (char)(32 + rand() % (126 - 32 + 1));
    buf[10] = '\0';
    return buf;
}

static const char *gen_hello(void)  { return "Hello!    "; }
static const char *gen_world(void)  { return "World!    "; }

/* -------------------------------------------------------------------------
 * Main
 * ---------------------------------------------------------------------- */

int main(void)
{
    int      i, n_cells;
    framebuf fb;
    Cell     cells[MAX_CELLS];

    srand((unsigned)time(NULL));

    /* ---- Open framebuffer and validate size -------------------------- */

    if (fb_open(&fb, "/dev/fb0") < 0) return 1;

    if (fb.w < WIN_X + SCREEN_W || fb.h < WIN_Y + SCREEN_H) {
        fprintf(stderr,
                "framebuffer too small: got %dx%d, need %dx%d\n",
                fb.w, fb.h, WIN_X + SCREEN_W, WIN_Y + SCREEN_H);
        return 1;
    }

    /* ---- Initialise atlases ------------------------------------------ */

    shapes_atlas *l_shapes = shapes_init(24,                                    RED,    TEAL);
    chars_atlas  *l_chars  = chars_init (24, 24,                                GREEN,  BROWN);
    mono_atlas   *l_mono   = mono_init  ("../ex2/RobotoMono-Regular-24.font",   PURPLE, OLIVE);
    tt_atlas     *l_tt     = tt_init    ("../ex2/Roboto-Regular-24.font",       BLUE,   SLATE);

    shapes_atlas *r_shapes = shapes_init(24,                                    VIOLET, TEAL);
    chars_atlas  *r_chars  = chars_init (24, 24,                                PINK,   BROWN);
    mono_atlas   *r_mono   = mono_init  ("../ex2/RobotoMono-Regular-12.font",   WHITE,  OLIVE);
    tt_atlas     *r_tt     = tt_init    ("../ex2/Roboto-Regular-12.font",       GRAY,   SLATE);

    if (!l_shapes || !l_chars || !l_mono || !l_tt ||
        !r_shapes || !r_chars || !r_mono || !r_tt) {
        fprintf(stderr, "atlas init failed\n");
        return 1;
    }

    /* ---- Describe the layout ----------------------------------------- */

    node *layout =
        COL(1,
            ROW(1,
                CELL(API_TT, r_tt, gen_hello),
                CELL(API_TT, r_tt, gen_world),
                CELL(API_TT, r_tt, gen_random),
                CELL(API_TT, r_tt, gen_random),
                CELL(API_TT, r_tt, gen_random)
            ),
            ROW(10,
                COL(1, CELL(API_SHAPES, l_shapes, gen_hello),
                       CELL(API_CHARS,  l_chars,  gen_hello),
                       CELL(API_MONO,   l_mono,   gen_hello),
                       CELL(API_TT,     l_tt,     gen_hello) ),
                COL(1, CELL(API_SHAPES, l_shapes, gen_random),
                       CELL(API_CHARS,  l_chars,  gen_random),
                       CELL(API_MONO,   l_mono,   gen_random),
                       CELL(API_TT,     l_tt,     gen_random) ),
                COL(1, CELL(API_SHAPES, r_shapes, gen_world),
                       CELL(API_CHARS,  r_chars,  gen_world),
                       CELL(API_MONO,   r_mono,   gen_world),
                       CELL(API_TT,     r_tt,     gen_random) ) )
        );

    /* ---- Resolve layout once — never again ---------------------------- */

    n_cells = resolve_node(cells, 0, layout,
                           WIN_X + MARGIN_X,
                           WIN_Y + MARGIN_Y,
                           SCREEN_W - 2 * MARGIN_X,
                           SCREEN_H - 2 * MARGIN_Y);

    /* ---- Draw border once -------------------------------------------- */

    draw_border(fb.mem, fb.stride, 4);

    /* ---- Render loop — zero layout math -------------------------------- */

    while (1) {
        for (i = 0; i < n_cells; i++)
            cell_draw(&cells[i], fb.mem, fb.stride);

        usleep(33333);
    }

    return 0;
}
