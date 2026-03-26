-- demo.lua — imgrids feature tutorial
-- Transpile: lua scripts/layout.lua < examples/demo.lua > examples/app/ui.rs

screen = {
	width = 800,
	height = 480,
}

colors = {
	white = { 255, 255, 255 },
	black = { 0, 0, 0 },
	red = { 255, 0, 0 },
	green = { 0, 255, 0 },
	blue = { 0, 0, 255 },
	yellow = { 255, 255, 0 },
}

fonts = {
	roboto = { "fonts/RobotoMono-Regular.ttf", 32 },
	small = { "fonts/RobotoMono-Regular.ttf", 20 },
	vga = { "raster::font_vga16", 12 },
}

style = {
	normal = {
		font = fonts.roboto,
		fg = colors.white,
		bg = colors.black,
		margin = 0,
		pad = 0,
		border = { width = 1, color = colors.white },
	},
	focused = {
		border = { width = 10 },
	},
}

-- Title bar: gray strip at top; explicitly override all inherited style
local function title(text)
	return {
		text,
		size = 40,
		bg = { 60, 60, 60 },
		fg = colors.white,
		font = fonts.small,
		border = { width = 0 },
		margin = 0,
	}
end

-- Navigation button: blue bg, white border, weight=2 so it takes more space than gaps
local function btn(label, target)
	return {
		label,
		weight = 2,
		press = { "nav", target },
		bg = { 30, 80, 180 },
		border = { width = 2, color = colors.white },
		margin = 4,
	}
end

-- Navigation row: fixed 60px, explicit style so menu-level style doesn't bleed in
local function nav(prev, next)
	return {
		"row",
		size = 60,
		bg = colors.black,
		fg = colors.white,
		font = fonts.roboto,
		margin = 0,
		border = { width = 0 },
		"",
		btn("Prev", prev),
		"",
		"",
		btn("Next", next),
		"",
	}
end

-- Description block: fixed height for 2 lines, no border, immune to menu-level style
-- Use \n to split into two lines.
local function desc(text)
	return {
		text,
		size = 50,
		border = { width = 0 },
		margin = 0,
		pad = 4,
		pad_left = 10,
		bg = colors.black,
		fg = colors.white,
		font = fonts.small,
	}
end

menus = {

	Hello = {
		"col",
		title("col layout"),
		desc("col stacks children vertically.\nEach child gets equal height."),
		nav("Complex", "Rows"),
		{ "First",  bg = { 60,  60,  120 } },
		{ "Second", bg = { 60,  120, 60  } },
		{ "Third",  bg = { 120, 60,  60  } },
	},

	Rows = {
		"col",
		title("row layout"),
		desc("row places children side by side.\n(equal width by default)"),
		nav("Hello", "Cols"),
		{ "row", "Left", "Center", "Right" },
		{ "row", "A", "B", "C", "D" },
	},

	Cols = {
		"col",
		title("col + row nesting"),
		desc("col inside a row:\neach column stacks its own children."),
		nav("Rows", "Weighted"),
		{
			"row",
			{ "col", "Col A top", "Col A mid", "Col A bot" },
			{ "col", "Col B top", "Col B mid", "Col B bot" },
			{ "col", "Col C top", "Col C mid", "Col C bot" },
		},
	},

	Weighted = {
		"col",
		title("weight="),
		desc("weight= sets proportional space.\nDefault weight is 1."),
		nav("Cols", "Fixed"),
		{ "row", "weight 1", { "weight 2", weight = 2 }, { "weight 3", weight = 3 } },
	},

	Fixed = {
		"col",
		title("size= (fixed)"),
		desc("size= gives a child a fixed pixel size.\nWeighted children share the rest."),
		nav("Weighted", "Styled"),
		{ "size=70 (fixed)", size = 70 },
		{ "weight=2 (weighted)", weight = 2 },
	},

	Styled = {
		"col",
		font = fonts.roboto,
		bg = colors.green,
		fg = colors.black,
		title("menu-level style"),
		desc("Style on the menu applies to all children.\nbg=, fg=, font=, pad=, margin=, border="),
		nav("Fixed", "SubStyled"),
		{ "row", "Green bg", "inherited by all" },
	},

	SubStyled = {
		"col",
		bg = colors.green,
		title("per-child style"),
		desc("Style on a child overrides\njust that child's subtree."),
		nav("Styled", "Pad"),
		{
			"row",
			{ "Blue", bg = colors.blue, fg = colors.white },
			"Green (inherited)",
			{ "Red", bg = colors.red, fg = colors.white },
		},
	},

	Pad = {
		"col",
		title("pad="),
		desc("pad= adds internal space.\npad_left/top/right/bottom: per-side."),
		nav("SubStyled", "Margin"),
		{ "row",
		  { "pad=5",  pad = 5  },
		  { "pad=15", pad = 15 },
		  { "pad_left=30", pad_left = 30 },
		},
		{ "row",
		  { "pad_top=20",    pad_top    = 20 },
		  { "pad_right=20",  pad_right  = 20 },
		  { "pad_bottom=20", pad_bottom = 20 },
		},
	},

	Margin = {
		"col",
		title("margin="),
		desc("margin= shrinks a child from outside.\n(all four sides equally)"),
		nav("Pad", "Borders"),
		{
			"row",
			"margin 0",
			{ "margin 20", margin = 20, bg = colors.blue },
			{ "margin 5", margin = 5, bg = colors.green },
		},
	},

	Borders = {
		"col",
		border = { width = 0 }, -- reset so the demos below are unambiguous
		title("border="),
		desc("border= draws a border.\nside= restricts to one edge."),
		nav("Margin", "Clickable"),
		{ "row", "no border", "no border", "no border" },
		{
			"row",
			{ "width=4\ncolor=green",  border = { width = 4, color = colors.green } },
			{ "width=4\nside=\"top\"",   border = { width = 4, side = "top" } },
			{ "width=4\nside=\"right\"", border = { width = 4, side = "right" } },
			{ "width=0",               border = { width = 0 } },
		},
		{ "row", "no border", "no border", "no border" },
		{ "width=3, color=yellow", border = { width = 3, color = colors.yellow } },
	},

	Clickable = {
		"col",
		title("press= callbacks"),
		desc("press={fn, args...} triggers\na Callbacks method on press."),
		nav("Borders", "Focusable"),
		{ 'press={"click"}',           press = { "click" } },
		{ 'press={"action","hello"}',  press = { "action", "hello" } },
		{ 'press={"action","a","b"}',  press = { "action", "a", "b" } },
	},

	Focusable = {
		"col",
		title("focusable="),
		desc("Focused cell redraws with style.focused.\nDefault: focusable iff press= is set."),
		nav("Clickable", "Dynamic"),
		{
			"row",
			{ "press=\nfocusable=true",  press = { "click" }, focusable = true  },
			{ "press=\nfocusable=false", press = { "click" }, focusable = false },
		},
		{
			"row",
			{ "label\nfocusable=true",  focusable = true  },
			{ "label\nfocusable=false", focusable = false },
		},
	},

	Dynamic = {
		"col",
		title("dynamic labels"),
		desc("lbl= cells get values via update_changes().\nThey start blank; populate on first update."),
		nav("Focusable", "Progress"),
		{ lbl = "parameter One" },
		{ lbl = "parameter Two" },
	},

	Progress = {
		"col",
		title("progress bar"),
		desc('render="progress bar":\nlbl= value is a float in [0,1].'),
		nav("Dynamic", "Popup"),
		{ lbl = "parameter One", render = "progress bar" },
		{ lbl = "parameter One", render = "progress bar",
		  pad = 8, fg = colors.green, bg = { 40, 40, 40 } },
		{ "row",
		  { lbl = "parameter One", render = "progress bar",
		    pad = 8, fg = colors.green, bg = { 40, 40, 40 } },
		  { lbl = "parameter One" },
		},
	},

	Popup = {
		"col",
		size = { math.floor(0.5 * screen.width), math.floor(0.7 * screen.height) },
		align = { math.floor(0.5 * screen.width), math.floor(0.5 * screen.height) },
		anchor = "center",
		title("popup / aligned"),
		desc("size={w,h}: menu size in pixels.\nalign={x,y}+anchor=: position."),
		nav("Progress", "Complex"),
	},

	Complex = {
		"col",
		title("nested containers"),
		desc("Containers nest freely:\ncol in row, row in col..."),
		nav("Popup", "Hello"),
		{
			"row",
			{ "col", "Top-left", "Bottom-left" },
			{ "col", { "row", "TR-A", "TR-B" }, { "row", "BR-A", "BR-B", "BR-C" } },
		},
	},
}
