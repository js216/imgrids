-- demo.lua — imgrids feature tutorial
-- Each menu demonstrates 1-3 features; navigate with Prev/Next.
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
   gray   = {128, 128, 128},
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

   -- Focused style: shown when a focusable cell is pressed.
   -- Only overrides fields you specify; rest inherits from normal.
   focused = {
      border = { width = 10 }, -- thick white border marks the focused cell
   },
}

menus = {

   -- ── 1. Static text, col layout ──────────────────────────────────────────
   -- "col" stacks children vertically; each child gets equal height by default.
   Hello = {"col",
      {"row", {"Prev", press={"nav", "Complex"}}, {"Next", press={"nav", "Rows"}}},
      "Static text in a column layout.",
      "Each child gets equal height.",
      "Text is vertically centered.",
   },

   -- ── 2. Row layout ───────────────────────────────────────────────────────
   -- "row" places children side by side; each child gets equal width.
   Rows = {"col",
      {"row", {"Prev", press={"nav", "Hello"}}, {"Next", press={"nav", "Weighted"}}},
      "Row layout: children side by side.",
      {"row", "Left cell", "Center cell", "Right cell"},
      {"row", "A", "B", "C", "D"},
   },

   -- ── 3. Weight attribute ─────────────────────────────────────────────────
   -- weight= controls how much space a child gets relative to its siblings.
   -- Default weight is 1. A child with weight=2 gets twice as much space.
   Weighted = {"col",
      {"row", {"Prev", press={"nav", "Rows"}}, {"Next", press={"nav", "Fixed"}}},
      "weight= controls proportional space.",
      {"row", "weight 1", "weight 2 (double)", weight=2},
      {"row", "1", "3 (triple)", weight=3, "1"},
   },

   -- ── 4. Fixed-size children ──────────────────────────────────────────────
   -- size= gives a child a fixed pixel size (height for col, width for row).
   -- Weighted children share the remaining space after fixed children.
   Fixed = {"col",
      {"row", {"Prev", press={"nav", "Weighted"}}, {"Next", press={"nav", "Styled"}}},
      "size= fixes a child's height (or width in row).",
      {"Little", size = 60},            -- exactly 60 px tall
      "Weighted (weight=1, gets rest)",
      {"Bigger", weight = 2},           -- twice as much as the default-weight row above
   },

   -- ── 5. Menu-level style ─────────────────────────────────────────────────
   -- Style attributes on the menu node apply to all children.
   -- bg and fg change background and foreground colors.
   Styled = {"col",
      font = fonts.roboto, bg = colors.green, fg = colors.black,
      {"row", {"Prev", press={"nav", "Fixed"}}, {"Next", press={"nav", "SubStyled"}}},
      "bg= and fg= apply to the whole menu.",
      "Children inherit parent style.",
      {"row", "All children", "see green bg"},
   },

   -- ── 6. Per-cell style override ──────────────────────────────────────────
   -- Style attributes on a child override only that child's style.
   SubStyled = {"col",
      bg = colors.green,
      {"row", {"Prev", press={"nav", "Styled"}}, {"Next", press={"nav", "Pad"}}},
      "Style on a child overrides just that child.",
      {"row",
         {"Blue cell", bg = colors.blue, fg = colors.white},
         "Green (inherited)",
         {"Red cell",  bg = colors.red,  fg = colors.white},
      },
   },

   -- ── 7. Padding ──────────────────────────────────────────────────────────
   -- pad= adds space inside a container between its border and its children.
   -- pad_left/top/right/bottom override individual sides.
   Pad = {"col", pad = 10,
      {"row", {"Prev", press={"nav", "SubStyled"}}, {"Next", press={"nav", "Margin"}}},
      "pad= adds internal space on all sides.",
      {"row",
         {"Big left pad", pad_left = 40},
         {"Bottom pad",   pad_bottom = 20},
      },
   },

   -- ── 8. Margin ───────────────────────────────────────────────────────────
   -- margin= adds external space outside a child, shrinking it from all sides.
   -- Unlike pad (which is internal), margin affects the child's own rect.
   Margin = {"col",
      {"row", {"Prev", press={"nav", "Pad"}}, {"Next", press={"nav", "Borders"}}},
      "margin= shrinks a child from all sides (external space).",
      {"row",
         {"margin 0 (normal)"},
         {"margin 20", margin = 20, bg = colors.blue},
         {"margin 5",  margin = 5,  bg = colors.green},
      },
   },

   -- ── 9. Borders ──────────────────────────────────────────────────────────
   -- border= draws a rectangular border. width= sets thickness, color= sets color.
   -- side= restricts to one edge: "top", "bottom", "left", or "right".
   Borders = {"col",
      {"row", {"Prev", press={"nav", "Margin"}}, {"Next", press={"nav", "Clickable"}}},
      "border= draws a border around a cell or container.",
      {"row",
         {"Full border",   border = {width = 4, color = colors.green}},
         {"Top only",      border = {width = 4, side = "top"}},
         {"Right only",    border = {width = 4, side = "right"}},
      },
      {"Row border (green)", border = {width = 3, color = colors.yellow}},
   },

   -- ── 10. Press callbacks ─────────────────────────────────────────────────
   -- press= {"fn_name", arg1, ...} triggers a Callbacks method on press.
   -- nav is the built-in: it switches menus by name.
   -- Custom callbacks are declared by any press= you use; the app implements them.
   Clickable = {"col",
      {"row", {"Prev", press={"nav", "Borders"}}, {"Next", press={"nav", "Focusable"}}},
      "press= triggers a callback method on touch/click.",
      {"Zero args — fn click()",          press = {"click"}},
      {"One arg  — fn action(args)",      press = {"action", "hello"}},
      {"Two args — fn action(args)",      press = {"action", "a", "b"}},
   },

   -- ── 11. Focusable cells ─────────────────────────────────────────────────
   -- focusable=true marks a cell as focusable (default: true if press= defined).
   -- Pressing a focusable cell highlights it with style.focused.
   -- focusable=false opts a cell out even if it has press=.
   Focusable = {"col",
      {"row", {"Prev", press={"nav", "Clickable"}}, {"Next", press={"nav", "Dynamic"}}},
      "Press a cell to focus it (thick border from style.focused).",
      {"Focusable — tap me!",       focusable = true},
      {"Not focusable (explicit)",  focusable = false},
      {"Also not focusable (no press, no focusable=true)"},
   },

   -- ── 12. Dynamic labels ──────────────────────────────────────────────────
   -- lbl= marks a cell as dynamic: its value is supplied at runtime via
   -- update_changes(&[("label name", "value")]).
   -- The cell is left blank on initial draw; populate it on the first update.
   Dynamic = {"col",
      {"row", {"Prev", press={"nav", "Focusable"}}, {"Next", press={"nav", "Progress"}}},
      "lbl= cells are updated at runtime via update_changes().",
      "Static text stays fixed.",
      {lbl = "parameter One"},   -- updated by app each frame
      {lbl = "parameter Two"},
   },

   -- ── 13. Progress bar ────────────────────────────────────────────────────
   -- render="progress bar" displays a lbl= cell as a horizontal bar.
   -- The value must be a float string in [0, 1] (e.g. "0.75").
   Progress = {"col",
      {"row", {"Prev", press={"nav", "Dynamic"}}, {"Next", press={"nav", "Popup"}}},
      "render=\"progress bar\" draws a filled bar (value in [0,1]).",
      {lbl = "parameter One", render = "progress bar"},
      {lbl = "parameter Two", render = "progress bar"},
   },

   -- ── 14. Popup / sized + aligned menu ────────────────────────────────────
   -- size= on the menu itself sets its pixel dimensions (default: full screen).
   -- align= sets the anchor point in screen coordinates.
   -- anchor= chooses which corner/center of the menu the align point refers to.
   Popup = {"col",
      size  = {math.floor(0.5 * screen.width), math.floor(0.7 * screen.height)},
      align = {math.floor(0.5 * screen.width), math.floor(0.5 * screen.height)},
      anchor = "center",
      {"row", {"Prev", press={"nav", "Progress"}}, {"Next", press={"nav", "Complex"}}},
      "size= shrinks the menu; align= positions it.",
      "anchor= chooses which point of the",
      "menu the align coordinate refers to.",
   },

   -- ── 15. Nested containers ───────────────────────────────────────────────
   -- Containers can be nested arbitrarily. A col inside a row inside a col, etc.
   -- Style inherits down the tree; overrides apply only to that subtree.
   Complex = {"col",
      {"row", {"Prev", press={"nav", "Popup"}}, {"Next", press={"nav", "Hello"}}},
      "Containers nest freely (col in row, row in col...).",
      {"row",
         {"col", "Top-left", "Bottom-left"},
         {"col",
            {"row", "Top-right A", "Top-right B"},
            {"row", "Bottom-right A", "Bottom-right B", "Bottom-right C"},
         },
      },
   },

}
