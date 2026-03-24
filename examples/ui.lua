-- GUI definition transpiled by a Lua program (stdin/stdout filter only) into a
-- Rust module that can be included in the application that needs a GUI

-- Transpiler passes:
--
-- 1. Collect — walk all menus, find every unique (font, fg, bg) combination
-- actually used (accounting for inheritance from style.normal). Each combo
-- needs one pre-baked atlas. Inherited styles need to be fully resolved during
-- this pass (TTF atlases bake fg/bg at creation time).
--
-- 2. Layout — recursively compute pixel rects for every cell. All inputs are
-- known at transpile time (screen.width, screen.height, weight, size, pad,
-- margin). Output is a flat list of {x, y, w, h, content, style, press, arg}
-- records per menu. No layout logic in the generated Rust.
--
-- 3. Emit — write the Rust module:
--
-- Generated API overview:
--
--   pub enum Menu { SimpleMenu, ClickableMenu, ... }  -- one variant per menu
--
--   pub fn draw(backend, menu, get_label)
--     Called once when switching to a menu. Draws everything from scratch.
--     get_label: fn(&str) -> &str  maps a label name to its current display
--     string; called once per dynamic cell during the initial draw only.
--
--   pub fn update(backend, menu, changes, events) -> Option<Menu>
--     Called every frame. changes: &[(&str, &str)] is a list of (label name,
--     new value) pairs for labels whose values have changed since last call;
--     the app is responsible for computing this diff. Only affected cells are
--     redrawn. events: &[InputEvent] — press events trigger focus changes and
--     press callbacks. Returns Some(next_menu) if a callback requested a menu
--     switch, None otherwise.
--
-- Typical app loop:
--
--   let mut menu = Menu::SimpleMenu;
--   draw(&mut backend, menu, get_label);
--   loop {
--       let changes = ...;  -- app computes changed labels
--       let events  = backend.poll_events();
--       if let Some(next) = update(&mut backend, menu, &changes, events) {
--           menu = next;
--           draw(&mut backend, menu, get_label);
--       }
--       sleep(33);
--   }
--
-- Implementation notes:
--
-- - One static atlas per font/color combo, initialised lazily at first draw.
-- - Per-cell value cache is a static mut array inside each update_<name>()
--   function; single-threaded so no locking needed.
-- - focused style: the layout pass resolves two full style sets (normal +
--   focused) per pressable cell; update_<name>() redraws the cell with the
--   appropriate style when focus changes.
-- - Fonts can be raster or TTF; atlases are loaded at runtime before the main
--   loop with negligible overhead.
-- - Dynamic cell redraw uses no fill_rect: the atlas already has the background
--   color baked in, so blit() overwrites every pixel in the cell in one pass.
--   To handle new text being shorter than old text, the transpiler computes at
--   transpile time the number of space characters needed to fill the cell width,
--   and the generated code pads the value string to that width before blitting.
--   No pixel is written twice.
-- - Multiple cell redraws in one update() call are batched inside a single
--   backend.render() closure, acquiring the framebuffer lock only once per
--   frame regardless of how many cells changed. Each atlas exposes blit(fb,
--   stride, x, y, text) for use inside render(); draw(backend, x, y, text) is
--   a convenience wrapper for single draws (e.g. during initial draw_()).
-- - Label name matching in update() uses integer IDs, not string comparisons:
--   the transpiler assigns a unique u32 to each distinct label name and emits
--   a changes: &[(u32, &str)] interface. The app maps its label names to these
--   IDs once at startup. Per-frame dispatch is a match on u32 — the compiler
--   emits a jump table, faster than any strcmp chain.

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
   -- note: fg/bg colors are baked into the anti-aliased font,
   -- so gui transpiler must bake one font for each bg/fg combo
   -- that is actually used in the gui being transpiled
   roboto = {"fonts/RobotoMono-Regular.ttf", 32}; -- ttf
   myriad = {"fonts/MyriadPro-Regular.ttf",  24}; -- also ttf
   vga    = {"raster::font_vga16", 12}; -- bitmap
   -- When an invalid font is given, like the vga17 below, issue WARNING and
   -- fallback to font_16 at 32-bit size --- big and ugly so the problem
   -- is immediately apparent
   vga17  = {"raster::font_vga17", 16};
}

style = {
   normal = {
      font = fonts.roboto,
      fg = colors.white, -- fg means the font foreground color
      bg = colors.black, -- bg is font background color as well as the widget color
      margin = 0, -- default unit for all dimensions: pixels
      pad = 0,
      border = {
         width = 0,
         color = colors.white,
      },
   },

   -- In the entire UI exactly zero or one buttons (i.e., widgets with a "press"
   -- attribute defined) can be focused, meaning they have the style defined
   -- here below. The widget gets focused whenever someone presses on it.
   focused = {
      border = {
         -- inherit white color, perfectly visible on black background
         width = 10,
      },
      -- all other properties same as the "normal" style (but designer can
      -- choose to override any of the other properties)
   },
}

-- each menu is transpiled into a single rust function (fn has same name as menu name);
-- app has a variable that keeps the current menu displayed;
-- callback functions can clear screen and substitute for a different menu
menus = {
   -- simple menu
   simple_menu = {
      "col", -- first item declares layout type: row or column
      "Item One", -- items without an explicit key are children
      font = fonts.roboto, -- key "font" given, so this is an attribute
      "Item Two", -- no key: this is also a child
      "Item One", -- duplication is permitted
      "More Text", -- text is always vertically centered in its cell
      -- if there's too many items that fit on the screen it just looks ugly
      -- (we do not provide any clipping or scrolling features)
   },

   -- static vs dynamic labels: Rust code must provide a function which is to
   -- be called with the string parameter name (lbl="this one!") and returns the
   -- string with the value formatted as a string
   dyn_stat_menu = {"col",
      "Simple Label", -- plain text label
      {lbl="parameter One"}, -- dynamic label
   },

   -- different ways of rendering a parameter
   widget_menu = {"col",
      "Simple Label", -- plain text label
      {lbl="parameter One"}, -- dynamic label, converted to text
      {lbl="parameter Two", render="progress bar"}, -- show as progress bar
      -- we can define lots of other kinds of "render" widgets later;
      -- for now, only "progress bar" is defined
   },

   -- 2x2 grid layout
   grid_menu = {"col",
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
   },

   -- popup menu
   popup_menu = {"col", size = {0.50*screen.width, 0.75*screen.height}, -- takes half screen width and 75% height
      align = {0.5 * screen.width, 0.5 * screen.height}, -- where to put the menu (default: 50%/50% = screen center)
      anchor = "center", -- what `align` is defined with respect to (default = center)
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
      {"row", "Label Five", "Label Six"},
      {"row", "Label Seven", "Label Eight"},
   },

   -- 2x3 grid layout with unequal sizes: "col" layout consists of rows;
   -- weight for a row means vertical weight, for col it means horizontal weight
   unequal_menu = {"col",
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four", weight=3}, -- more vertical size than upper row
      {"row", "Label Five", "Label Six", weight=2}, -- bigger than top row and smaller than bottom row
   },

   -- styling that applies to the whole menu
   styled_menu = {"col",
      font=fonts.roboto, bg=colors.green, -- unfocused state
      focused={bg=colors.red,}, -- focused state
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
   },

   -- styling can apply to just one item
   sub_styled_menu = {"col", font=fonts.roboto, bg=colors.green,
      {"row", "Label One", "Label Two"},
      {"row",
         {"Label Three", font=fonts.myriad}, -- 1st elem of tbl not "row" or "col" -> leaf node
         "Label Four"},
   },

   -- three buttons one above another
   clickable_menu = {"col",
      {"Click me!", press = "function_cl"}, -- function name is raw string;
      {"Press me!", press = "function_pr"}, -- it corresponds to a function
      {"Touch me!", press = "function_to"}, -- defined in a Rust file elsewhere;
      {"Test!",     press = "fn2", arg="string arg"}, -- pure static string, nothing else
      {"Test2!",    press = "fn2"}, -- "" if arg missing
      {lbl="parameter One", press="fn3"}, -- dynamic label that's clicable
   },

   -- more complex grid layout (could be nested to arbitrary depth)
   complex_menu = {"row",
      {"col", "Label One", "Label Two"},
      {"col",
         {"row", "Label Three", "Label Four"},
         {"row", "Label Five", "Label Six", "Label Seven"},
         {"row", "Click!", press="click"},
      },
   },

   -- any element can have padding (default unit: pixels)
   pad_menu = {"col", pad = 10, -- padding on all sides around the menu
      {"row", {"Label One", pad_left = 3}, "Label Two"}, -- left padding for one item only
      {"row", "Label Three", "Label Four", pad_bottom = 10}, -- bottom padding for the whole row
   },

   -- margin is external space (vs padding = internal space)
   margin_menu = {"col", margin = 5, -- outer margin for the menu (not children!)
      {"row", "Label One", "Label Two"},
      {"row", "Label Three", "Label Four"},
      {"row", "Label Five", bg=colors.green}, -- margin is NOT green, only the inside of the row
   },

   -- borders
   borders_menu = {"col",
      {"row", "Label One", "Label Two", border = {width = 3, color = colors.green}}, -- 3 px wide green border around the row
      {"row", "Label Three", {"Label Four", border = {width = 2}}}, -- just one cell has the border
      {"row", "Label Five", border = {width = 3, side = "top"}}, -- border only on top side
      {"row", "Label Six", border = {side = "top"}}, -- print WARNING: zero-width border
      {"row", "Label Seven", bg=colors.green}, -- whole row is green INCLUDING the padding
   },
}
