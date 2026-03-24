screen = {
   width  = 800,
   height = 480,
}

colors = {
   white = {255, 255, 255},
   black = {0, 0, 0},
   red   = {255, 0, 0},
   green = {0, 255, 0},
   blue  = {0, 0, 255},
}

fonts = {
   roboto = {"fonts/RobotoMono-Regular.ttf", 32};
   myriad = {"fonts/MyriadPro-Regular.ttf",  32};
   -- note: fg/bg colors are baked into the anti-aliased font,
   -- so gui compiler must bake one font for each bg/fg combo used
}

defaults = {
   font = fonts.roboto,
   fg = colors.white,
   bg = colors.black,
   margin = 0, -- default unit for all dimensions: pixels
   pad = 0,
   border = {
      width = 0,
      color = colors.white,
   },
}

menus = {
   -- two-item menu
   simple_menu = {
      "col", -- first item declares layout type: row or column
      "Item One", -- items without an explicit key are children
      font = roboto, -- key "font" given, so this is an attribute
      "Item Two", -- no key: this is also a child
      "Item One", -- duplication is permitted
   },

   -- static vs dynamic labels
   dyn_stat_menu = {"col",
      "Simple Label", -- plain text label
      {lbl="parameter One"}, -- dynamic label
   },

   -- different ways of rendering a parameter
   dyn_stat_menu = {"col",
      "Simple Label", -- plain text label
      {lbl="parameter One"}, -- dynamic label, converted to text
      {lbl="parameter Two", render="progress bar"}, -- show as progress bar
      --- we can define lots of other kinds of "render" widgets later
   },

   -- 2x2 grid layout
   simple_menu = {"col",
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
   },

   -- popup menu
   simple_menu = {"col", size = {50, 75}, -- takes half screen width and 75% height
      align = (50 * pct_x, 50 * pct_y), -- where to put the menu (default: 50%/50% = screen center)
      anchor = "center", -- what `align` is define with respect to (default = center)
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
      {"row", "Label Five", "Label Six"},
      {"row", "Label Seven", "Label Eight"},
   },

   -- 2x3 grid layout with unequal sizes
   simple_menu = {"col",
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four", weight=3}, -- more vertical size than upper row
      {"row", "Label Five", "Label Six", weight=2}, -- bigger than top row and smaller than bottom row
   },

   -- styling that applies to the whole menu
   styled_menu = {"col", font=roboto, bg=green
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
   },

   -- styling can apply to just one item
   sub_styled_menu = {"col", font=roboto, bg=green
      {"row", "Label One", "Label Two"},
      {"row",
         {"Label Three", font=myriad}, -- 1st elem of tbl not "row" or "col" -> leaf node
         "Label Four"},
   },

   -- three buttons one above another
   clickable_menu = {"col",
      {"row", "Click me!", press = "function_cl"},
      {"row", "Press me!", press = "function_pr"},
      {"row", "Touch me!", press = "function_to"},
   },

   -- more complex grid layout (could be nested to arbitrary depth)
   complex_menu = {"row",
      {"col", "Label One", "Label Two"},
      {"col",
         {"row", "Label Three", "Label Four"},
         {"row", "Label Five", "Label Six", "Label Seven"},
         {"row", "Click!", press="click"},
   },

   -- any element can have padding (default unit: pixels)
   simple_menu = {"col", pad = 10, -- padding on all sides around the menu
      {"row", {"Label One", pad_left = 3}, "Label Two"}, -- left padding for one item only
      {"row", "Label Three", "Label Four", pad_bottom = 10}, -- bottom padding for the whole row
   },

   -- margin is external space (vs padding = internal space)
   simple_menu = {"col", margin = 5, -- outer margin for the menu (not children!)
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
      {"row", "Label Five", bg=colors.green}, -- margin is NOT green, only the inside of the row
   },

   -- borders
   simple_menu = {"col",
      {"row", "Label One", "Label Two", border = {width = 3, color = "green"}}, -- 3% wide green border around the row
      {"row", "Label Three", {"Label Four", border = {width = 2}}}, -- just one cell has the border
      {"row", "Label Five", border = {width = 3, side = "upper"}}, -- border only on top side
      {"row", "Label Six", border = {side = "upper"}}, -- print WARNING: zero-width border
      {"row", "Label Seven", bg=colors.green}, -- whole row is green INCLUDING the padding
   },
}
