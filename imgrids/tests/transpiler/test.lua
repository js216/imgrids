-- Test input that exercises the error conditions causing immediate nonzero exit.
-- Each section is a separate scenario; only the first will execute
-- (transpiler exits on first error).

-- (1) Non-PascalCase menu name
screen = { width = 800, height = 480 }
colors = { black = {0, 0, 0}, white = {255, 255, 255} }
fonts  = { vga = {"raster::font_vga16", 16} }
style  = { normal = { font = fonts.vga, fg = colors.white, bg = colors.black } }
menus  = {
    bad_name = { "col", "Hello" },   -- ERROR: not PascalCase
}
