/*
 * shapes.h — Geometric shapes renderer (squares, circles, triangles).
 */

#ifndef SHAPES_H
#define SHAPES_H

#include <stdint.h>

typedef uint32_t pixel_t;

typedef struct shapes_atlas shapes_atlas;

/*
 * shapes_init — rasterise a 128-slot atlas of shapes at cell_size × cell_size.
 *   fg : foreground (shape fill) colour — 0xAARRGGBB
 *   bg : background colour              — 0x00000000 = transparent
 */
shapes_atlas *shapes_init(int cell_size, pixel_t fg, pixel_t bg);

/*
 * shapes_draw — blit a NUL-terminated string of shape glyphs into the
 * framebuffer at (x, y).  Each character's ASCII value indexes the atlas.
 */
void shapes_draw(const shapes_atlas *atlas,
                 pixel_t *fb, int fb_stride,
                 int x, int y,
                 const char *text);

int shapes_cell_size(const shapes_atlas *atlas);

#endif /* SHAPES_H */
