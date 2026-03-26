-- Test input that exercises all four warning conditions.
-- Expected: transpiler finishes (produces valid Rust output) but exits 1.

screen = { width = 800, height = 480 }
colors = { black = {0, 0, 0}, white = {255, 255, 255} }
fonts = {
    vga     = {"raster::font_vga16", 16},
    bad     = {"raster::font_vga17", 16},  -- (1) unknown raster font
}
style = {
    normal = { font = fonts.vga, fg = colors.white, bg = colors.black },
}
menus = {
    Warn = { "col",
        size   = {400, 300},
        align  = {400, 240},
        anchor = "bad_anchor",              -- (2) unknown anchor
        {"Click",  press = "click"},        -- (3) old press syntax
        {"row", "Label", border = {side = "top"}},  -- (4) zero-width border
        {"BadFont", font = fonts.bad},      -- (1) unknown raster font used
        {"Big margin", margin = 500},       -- (5) margin exceeds available size
    },
}
