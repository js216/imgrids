# Towards a fast, immediate-mode grid-layout software-rendered GUI

This repository contains a small sequence of programs to answer the question,
just how fast can one render a simple, mostly text-based GUI, direct to the
framebuffer?

- ex1: Draw a random sequence of symbols to fill the whole screen
- ex2: Write the symbols to a particular location on the screen
- ex3: Automatic grid layout, quasi domain-specific language

### Example 1: Writing text to a framebuffer

These examples draw a random sequence of symbols to a framebuffer (see
screenshots [here](ex1/README.md)).

- shapes: draw random colored shapes across the screen
- chars: draw scaled up characters from a simple monospaced font
- mono: treat the TTF font symbols as if they are monospaced
- tt: draw symbols derived from a proportional TTF font

The maximum CPU usage (Pentium N3540 @ 2.16GHz) depends on the font size:

| Program    |     12pt |     24pt |     36pt |     64pt |
| ---------- | -------- | -------- | -------- | -------- |
| shapes     |     6.0% |     5.0% |     4.0% |     3.0% |
| chars      |     7.0% |     5.0% |     5.0% |     3.0% |
| mono       |    11.0% |     8.0% |     6.0% |     6.0% |
| tt         |    13.0% |     9.0% |     6.0% |     7.0% |

### Example 2: Writing text to a particular location

We now rewrite the renderers from the previuos example to write to a particular
location on the screen. This is a two step process: first, a `_init()` function
creates the "font atlas", and then, in the main loop, the `_draw()` function
draws the text wherever we want it to.

| Renderer | `_init`                                                        | `_draw`         |
|----------|----------------------------------------------------------------|-----------------|
| `chars`  | `chars_init(int glyph_w, int glyph_h, pixel_t fg, pixel_t bg)` | `chars_draw()`  |
| `shapes` | `shapes_init(int cell_size, pixel_t fg, pixel_t bg)`           | `shapes_draw()` |
| `mono`   | `mono_init(const char *path, pixel_t fg, pixel_t bg)`          | `mono_draw()`   |
| `tt`     | `tt_init(const char *path, pixel_t fg, pixel_t bg)`            | `tt_draw()`     |

The `_draw` functions all the the same call signature:

```
void _draw(atlas, pixel_t *fb, int fb_stride, int x, int y, const char *text);
```

The arguments are as follows:

- `atlas`     : opaque handle returned by the corresponding `_init`
- `fb`        : pointer to pixel (0,0) of the framebuffer
- `fb_stride` : pixels per scanline (may exceed visible width)
- `x`, `y`    : top-left pixel of the first glyph
- `text`      : NUL-terminated ASCII string to render

For `_init` functions, arguments are as follows:

- `glyph_w,` `glyph_h`: cell size in pixels (`chars` only)
- `cell_size`: cell width and height in pixels (`shapes` only)
- `path`: path to `.font` file from `font_to_bin.py` (`mono` and `tt` only)
- `fg`: foreground (text/shape fill) colour, 0xAARRGGBB
- `bg`: background colour, 0xAARRGGBB; use 0x00000000 for transparent

The combined [demo](ex2/demo.c) takes up about 1% CPU.

### Example 3: Automatic grid layouts

Instead of manually calculating where each text should go on the screen, it
would be nice to write something like:

```c
ROW(1,
    COL(1, CELL(API_TT, my_atlas, gen_label),
           CELL(API_TT, my_atlas, gen_value) ),
    COL(1, CELL(API_TT, my_atlas, gen_label),
           CELL(API_TT, my_atlas, gen_value) )
)
```

Here `CELL(api, atlas, gen)` is a leaf node: `api` selects the renderer
(`API_TT`, `API_SHAPES`, `API_MONO`, `API_CHARS`), `atlas` is the
pre-initialised font atlas, and `gen` is a `const char *(*)(void)` function
pointer — a functor — called each frame to produce the text to display. A `gen`
function can return a compile-time string literal, format a value from a shared
buffer, or generate random content; the layout machinery does not care.

`ROW(weight, ...)` and `COL(weight, ...)` are internal nodes that divide their
bounding box horizontally or vertically among their children. The integer weight
controls the proportional share of space each child receives relative to its
siblings: a child with weight 2 gets twice the space of a child with weight 1.
Nesting is arbitrary — a `ROW` can contain `COL`s that contain further `ROW`s.

The window geometry is controlled by four defines:

```c
#define WIN_X    0      /* left edge within framebuffer  */
#define WIN_Y    0      /* top  edge within framebuffer  */
#define SCREEN_W 800    /* window width  in pixels       */
#define SCREEN_H 480    /* window height in pixels       */
```

and two margin defines that set the gap between the border and the grid:

```c
#define MARGIN_X 20
#define MARGIN_Y 20
```

**Zero overhead guarantee.** The `ROW`/`COL`/`CELL` macros produce a tree of
compound literals that exists only before the render loop. `resolve_node()`
walks the tree exactly once, computes every cell's `(x, y)` position, and writes
a flat `Cell[]` array. The tree is never touched again. The render loop is a
single `for` over that array with one function-pointer call per cell — identical
in cost to what you would have written by hand.

**Dirty-cell optimisation.** Each `Cell` caches the last string it rendered.
`cell_draw()` calls `gen()`, compares the result against the cache with
`strncmp`, and skips the pixel blit entirely if the text has not changed. Static
labels therefore cost one comparison per frame rather than a full redraw, which
roughly halves CPU usage when most of the screen is static text.

### Author

Jakob Kastelic
