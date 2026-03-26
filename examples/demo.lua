-- demo.lua — imgrids feature tutorial
-- Transpile: lua scripts/layout.lua < examples/demo.lua > examples/app/ui.rs

screen = {
   width  = 800,
   height = 480,
}

colors = {
   white  = {255, 255, 255},
   black  = {0,   0,   0  },
   red    = {255, 0,   0  },
   green  = {0,   255, 0  },
   blue   = {0,   0,   255},
   yellow = {255, 255, 0  },
}

fonts = {
   roboto = {"fonts/RobotoMono-Regular.ttf", 32},
   small  = {"fonts/RobotoMono-Regular.ttf", 20},
   vga    = {"raster::font_vga16", 12},
}

style = {
   normal = {
      font   = fonts.roboto,
      fg     = colors.white,
      bg     = colors.black,
      margin = 0,
      pad    = 0,
      border = { width = 1, color = colors.white },
   },
   focused = {
      border = { width = 10 },
   },
}

-- Title bar: gray strip at the top of each menu
local function title(text)
   return {text, size = 40, bg = {60, 60, 60}, fg = colors.white,
           font = fonts.small, border = {width = 0}}
end

-- Navigation button: blue bg, white border, small margin
local function btn(label, target)
   return {label,
      press  = {"nav", target},
      bg     = {30, 80, 180},
      border = {width = 2, color = colors.white},
      margin = 4,
   }
end

menus = {

   Hello = {"col",
      title("1. col layout"),
      {"row", size = 60, btn("Prev", "Complex"), btn("Next", "Rows")},
      "col stacks children vertically.",
      "Each child gets equal height.",
      "Text is vertically centered.",
   },

   Rows = {"col",
      title("2. row layout"),
      {"row", size = 60, btn("Prev", "Hello"), btn("Next", "Cols")},
      "row places children side by side.",
      "(equal width by default)",
      {"row", "Left", "Center", "Right"},
      {"row", "A", "B", "C", "D"},
   },

   Cols = {"col",
      title("3. col + row nesting"),
      {"row", size = 60, btn("Prev", "Rows"), btn("Next", "Weighted")},
      "col inside a row:",
      "each column stacks its own children.",
      {"row",
         {"col", "Col A top", "Col A mid", "Col A bot"},
         {"col", "Col B top", "Col B mid", "Col B bot"},
         {"col", "Col C top", "Col C mid", "Col C bot"},
      },
   },

   Weighted = {"col",
      title("4. weight="),
      {"row", size = 60, btn("Prev", "Cols"), btn("Next", "Fixed")},
      "weight= sets proportional space.",
      "Default weight is 1.",
      {"row", "weight 1", {"weight 2", weight=2}, {"weight 3", weight=3}},
   },

   Fixed = {"col",
      title("5. size= (fixed)"),
      {"row", size = 60, btn("Prev", "Weighted"), btn("Next", "Styled")},
      "size= gives a child a fixed pixel size.",
      {"For col: size= is height in pixels.", size = 70},
      "Weighted children share the rest.",
      {"Bigger (weight 2)", weight = 2},
   },

   Styled = {"col",
      font = fonts.roboto, bg = colors.green, fg = colors.black,
      title("6. menu-level style"),
      {"row", size = 60, btn("Prev", "Fixed"), btn("Next", "SubStyled")},
      "Style on the menu applies to all children.",
      "bg=, fg=, font=, pad=, margin=, border=",
      {"row", "Green bg", "inherited by all"},
   },

   SubStyled = {"col",
      bg = colors.green,
      title("7. per-child style"),
      {"row", size = 60, btn("Prev", "Styled"), btn("Next", "Pad")},
      "Style on a child overrides",
      "just that child's subtree.",
      {"row",
         {"Blue", bg = colors.blue, fg = colors.white},
         "Green (inherited)",
         {"Red",  bg = colors.red,  fg = colors.white},
      },
   },

   Pad = {"col", pad = 10,
      title("8. pad="),
      {"row", size = 60, btn("Prev", "SubStyled"), btn("Next", "Margin")},
      "pad= adds internal space.",
      "pad_left/top/right/bottom: per-side.",
      {"row",
         {"big pad_left", pad_left = 60},
         {"pad_bottom",   pad_bottom = 30},
      },
   },

   Margin = {"col",
      title("9. margin="),
      {"row", size = 60, btn("Prev", "Pad"), btn("Next", "Borders")},
      "margin= shrinks a child from outside.",
      "(all four sides equally)",
      {"row",
         "margin 0",
         {"margin 20", margin = 20, bg = colors.blue},
         {"margin 5",  margin = 5,  bg = colors.green},
      },
   },

   Borders = {"col",
      border = {width = 0},  -- reset so the demos below are unambiguous
      title("10. border="),
      {"row", size = 60, btn("Prev", "Margin"), btn("Next", "Clickable")},
      "border= draws a border.",
      "side= restricts to one edge.",
      {"row",
         {"Full border",   border = {width = 4, color = colors.green}},
         {"Top only",      border = {width = 4, side = "top"}},
         {"Right only",    border = {width = 4, side = "right"}},
         {"No border",     border = {width = 0}},
      },
      {"Whole-row border", border = {width = 3, color = colors.yellow}},
   },

   Clickable = {"col",
      title("11. press= callbacks"),
      {"row", size = 60, btn("Prev", "Borders"), btn("Next", "Focusable")},
      "press={fn, args...} triggers",
      "a Callbacks method on press.",
      {"fn click()  — zero args",    press = {"click"}},
      {"fn action() — one arg",      press = {"action", "hello"}},
      {"fn action() — two args",     press = {"action", "a", "b"}},
   },

   Focusable = {"col",
      title("12. focusable="),
      {"row", size = 60, btn("Prev", "Clickable"), btn("Next", "Dynamic")},
      "Press to focus (style.focused).",
      "Default: focusable iff press= is set.",
      {"Focusable (tap me!)",          focusable = true},
      {"Not focusable (explicit)",     focusable = false},
   },

   Dynamic = {"col",
      title("13. dynamic labels"),
      {"row", size = 60, btn("Prev", "Focusable"), btn("Next", "Progress")},
      "lbl= cells get values via update_changes().",
      "They start blank; populate on first update.",
      {lbl = "parameter One"},
      {lbl = "parameter Two"},
   },

   Progress = {"col",
      title("14. progress bar"),
      {"row", size = 60, btn("Prev", "Dynamic"), btn("Next", "Popup")},
      "render=\"progress bar\":",
      "lbl= value is a float in [0,1].",
      {lbl = "parameter One", render = "progress bar"},
      {lbl = "parameter Two", render = "progress bar"},
   },

   Popup = {"col",
      size  = {math.floor(0.5 * screen.width), math.floor(0.7 * screen.height)},
      align = {math.floor(0.5 * screen.width), math.floor(0.5 * screen.height)},
      anchor = "center",
      title("15. popup / aligned"),
      {"row", size = 60, btn("Prev", "Progress"), btn("Next", "Complex")},
      "size= and align= position",
      "a sub-screen menu.",
      "anchor= is which point of the menu",
      "the align= coord refers to.",
   },

   Complex = {"col",
      title("16. nested containers"),
      {"row", size = 60, btn("Prev", "Popup"), btn("Next", "Hello")},
      "Containers nest freely:",
      "col in row, row in col...",
      {"row",
         {"col", "Top-left", "Bottom-left"},
         {"col",
            {"row", "TR-A", "TR-B"},
            {"row", "BR-A", "BR-B", "BR-C"},
         },
      },
   },

}
