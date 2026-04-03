-- Test: accessing an undefined font exits immediately with an error.
screen = { width = 100, height = 100 }
colors = { white = {255, 255, 255} }
fonts = { f = {"raster::font_vga16", 16} }
style = { normal = { font = fonts.missing, fg = colors.white, bg = colors.white } }
menus = { T = {"col", "hi"} }
