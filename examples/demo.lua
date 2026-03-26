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
      border = { width = 0, color = colors.white },
   },
   focused = {
      border = { width = 10 },
   },
}

menus = {

   Hello = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Complex"}}, {"Next", press={"nav", "Rows"}}},
      "col stacks children vertically.",
      "Each child gets equal height.",
      "Text is vertically centered in its cell.",
   },

   Rows = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Hello"}}, {"Next", press={"nav", "Cols"}}},
      "row places children side by side (equal width).",
      {"row", "Left", "Center", "Right"},
      {"row", "A", "B", "C", "D"},
   },

   Cols = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Rows"}}, {"Next", press={"nav", "Weighted"}}},
      "col inside a row: each column stacks its own children.",
      {"row",
         {"col", "Col A top", "Col A mid", "Col A bot"},
         {"col", "Col B top", "Col B mid", "Col B bot"},
         {"col", "Col C top", "Col C mid", "Col C bot"},
      },
   },

   Weighted = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Cols"}}, {"Next", press={"nav", "Fixed"}}},
      "weight= controls proportional space (default 1).",
      {"row", "weight 1", {"weight 2", weight=2}, {"weight 3", weight=3}},
   },

   Fixed = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Weighted"}}, {"Next", press={"nav", "Styled"}}},
      "size= gives a child a fixed pixel size.",
      {"For col: size= is height in pixels.", size = 80},
      "Remaining space is shared by weighted children.",
      {"Bigger (weight 2)", weight = 2},
   },

   Styled = {"col",
      font = fonts.roboto, bg = colors.green, fg = colors.black,
      {"row", size = 60, {"Prev", press={"nav", "Fixed"}}, {"Next", press={"nav", "SubStyled"}}},
      "Style on the menu node applies to all children.",
      "bg=, fg=, font=, pad=, margin=, border= all work.",
      {"row", "Green bg", "inherited by all"},
   },

   SubStyled = {"col",
      bg = colors.green,
      {"row", size = 60, {"Prev", press={"nav", "Styled"}}, {"Next", press={"nav", "Pad"}}},
      "Style on a child overrides just that subtree.",
      {"row",
         {"Blue", bg = colors.blue, fg = colors.white},
         "Green (inherited)",
         {"Red",  bg = colors.red,  fg = colors.white},
      },
   },

   Pad = {"col", pad = 10,
      {"row", size = 60, {"Prev", press={"nav", "SubStyled"}}, {"Next", press={"nav", "Margin"}}},
      "pad= adds internal space between border and children.",
      "pad_left/top/right/bottom override individual sides.",
      {"row",
         {"big pad_left", pad_left = 60},
         {"pad_bottom",   pad_bottom = 30},
      },
   },

   Margin = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Pad"}}, {"Next", press={"nav", "Borders"}}},
      "margin= shrinks a child from the outside (all sides).",
      {"row",
         "margin 0",
         {"margin 20", margin = 20, bg = colors.blue},
         {"margin 5",  margin = 5,  bg = colors.green},
      },
   },

   Borders = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Margin"}}, {"Next", press={"nav", "Clickable"}}},
      "border= draws a border. side= restricts to one edge.",
      {"row",
         {"Full border",   border = {width = 4, color = colors.green}},
         {"Top only",      border = {width = 4, side = "top"}},
         {"Right only",    border = {width = 4, side = "right"}},
      },
      {"Whole-row border", border = {width = 3, color = colors.yellow}},
   },

   Clickable = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Borders"}}, {"Next", press={"nav", "Focusable"}}},
      "press={fn, args...} calls a Callbacks method on press.",
      {"fn click()  — zero args",    press = {"click"}},
      {"fn action() — one arg",      press = {"action", "hello"}},
      {"fn action() — two args",     press = {"action", "a", "b"}},
   },

   Focusable = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Clickable"}}, {"Next", press={"nav", "Dynamic"}}},
      "focusable=true: pressing highlights with style.focused.",
      "Default: focusable iff press= is defined.",
      {"Focusable (tap me!)",          focusable = true},
      {"Not focusable (explicit)",     focusable = false},
   },

   Dynamic = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Focusable"}}, {"Next", press={"nav", "Progress"}}},
      "lbl= cells receive values via update_changes() at runtime.",
      {lbl = "parameter One"},
      {lbl = "parameter Two"},
   },

   Progress = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Dynamic"}}, {"Next", press={"nav", "Popup"}}},
      "render=\"progress bar\": lbl= value is a float in [0,1].",
      {lbl = "parameter One", render = "progress bar"},
      {lbl = "parameter Two", render = "progress bar"},
   },

   Popup = {"col",
      size  = {math.floor(0.5 * screen.width), math.floor(0.7 * screen.height)},
      align = {math.floor(0.5 * screen.width), math.floor(0.5 * screen.height)},
      anchor = "center",
      {"row", size = 60, {"Prev", press={"nav", "Progress"}}, {"Next", press={"nav", "Complex"}}},
      "size= and align= position a sub-screen menu.",
      "anchor= is the menu point that align= refers to.",
   },

   Complex = {"col",
      {"row", size = 60, {"Prev", press={"nav", "Popup"}}, {"Next", press={"nav", "Hello"}}},
      "Containers nest freely: col in row, row in col...",
      {"row",
         {"col", "Top-left", "Bottom-left"},
         {"col",
            {"row", "TR-A", "TR-B"},
            {"row", "BR-A", "BR-B", "BR-C"},
         },
      },
   },

}
