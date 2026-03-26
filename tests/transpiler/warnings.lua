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
        menu_size   = {400, 300},
        menu_align  = {400, 240},
        menu_anchor = "bad_anchor",         -- (2) unknown anchor
        {"Click",  press = "click"},        -- (3) old press syntax
        {"row", "Label", border = {side = "top"}},  -- (4) zero-width border
        {"BadFont", font = fonts.bad},      -- (1) unknown raster font used
        {"Big margin", margin = 500},       -- (5) margin exceeds available size
        {"bad node key", blah = 1},       -- (7) unknown key in node
        {"bad style key", style = {size = 60}},  -- (8) unknown key in style=
        {"bad border key", border = {width = 1, radius = 5}}, -- (9) unknown key in border=
    },
    -- (6) text exceeds screen bounds: tall fixed child leaves only 10px
    -- for the last child, but the 16px font can't fit, overflows screen
    Overflow = { "col",
        {"Top",    size = 470},
        {"Bottom"},                            -- 10px tall, 16px font overflows
    },
}
