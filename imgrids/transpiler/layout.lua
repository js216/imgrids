#!/usr/bin/env lua
-- layout.lua — transpile ui.lua to a Rust imgrids UI module
-- Usage: lua scripts/layout.lua < examples/ui.lua > examples/app/ui.rs

-------------------------------------------------------------------------------
-- 1. Execute input
-------------------------------------------------------------------------------
local src = io.read("*a")
local chunk, err = load(src)
if not chunk then
	io.stderr:write("ERROR: " .. tostring(err) .. "\n")
	os.exit(1)
end
-- Inject strict metatables so that accessing an undefined key on
-- colors or fonts (e.g. colors.gren) errors immediately during chunk().
local function _strict_mt(name)
	return {__index = function(_, k)
		io.stderr:write(("ERROR: %s.%s is not defined\n"):format(name, tostring(k)))
		os.exit(1)
	end}
end
-- Pre-create globals so user code populates them; we intercept the
-- assignment via a global metatable that auto-applies strictness.
local _orig_globals_mt = getmetatable(_G)
setmetatable(_G, {
	__newindex = function(t, k, v)
		rawset(t, k, v)
		if (k == "colors" or k == "fonts") and type(v) == "table" then
			setmetatable(v, _strict_mt(k))
		end
	end,
	__index = _orig_globals_mt and _orig_globals_mt.__index or nil,
})
chunk()
setmetatable(_G, _orig_globals_mt)

-- Build param metadata lookup from global `params` table (loaded from params.txt)
local param_info = {}  -- name → {opts={...}, prec=N, unit="...", is_angle=bool}
if type(params) == "table" then
	for _, p in ipairs(params) do
		if p.name then
			local info = {}
			if p.opts and #p.opts > 0 then info.opts = p.opts end
			if p.prec then info.prec = p.prec end
			if p.unit then
				info.unit = p.unit
				info.is_angle = p.unit:sub(1,2) == "\u{00B0}" or p.unit:byte(1) == 0xC2 and p.unit:byte(2) == 0xB0
			end
			if next(info) then param_info[p.name] = info end
		end
	end
end

local warn_count = 0
local function warn(fmt, ...)
	io.stderr:write(("WARNING: " .. fmt .. "\n"):format(...))
	warn_count = warn_count + 1
end

-------------------------------------------------------------------------------
-- 2. Utilities
-------------------------------------------------------------------------------

-- Menu names must be PascalCase: they are used verbatim as Rust enum variant
-- names.  A valid PascalCase identifier starts with an uppercase ASCII letter
-- and contains only ASCII letters and digits.
local function check_menu_name(name)
	if not name:match("^[A-Z][A-Za-z0-9]*$") then
		io.stderr:write(("ERROR: menu name %q is not PascalCase.\n"):format(name))
		io.stderr:write("  Menu names are used verbatim as Rust enum variant names and must\n")
		io.stderr:write("  start with an uppercase letter and contain only letters and digits.\n")
		io.stderr:write(
			("  Suggestion: rename it to %q or similar.\n"):format(
				name:sub(1, 1):upper() .. name:sub(2):gsub("_(%a)", string.upper):gsub("_", "")
			)
		)
		os.exit(1)
	end
end

local function rgb_lit(c)
	return ("Pixel::from_rgb(%d, %d, %d)"):format(c[1], c[2], c[3])
end

-- Font can be {"path", size} (single) or {{"path1", size}, {"path2", size}} (fallback chain)
local function is_font_chain(font)
	return type(font[1]) == "table"
end

local function is_raster(font)
	if is_font_chain(font) then return false end
	return type(font[1]) == "string" and font[1]:sub(1, 7) == "raster:"
end

local function raster_mod(font)
	return font[1]:match("raster::(.+)")
end

-- Known raster fonts: {natural_w, natural_h}
local RASTER_DIMS = {
	font_vga16 = { 8, 16 },
	font8x8 = { 8, 8 },
	font_terminus_8x16 = { 8, 16 },
}

-- Fallback for unknown raster fonts (big and ugly so the problem is obvious)
local RASTER_FALLBACK = "font_vga16"
local RASTER_FALLBACK_SIZE = 32

local function resolve_raster_font(font)
	local name = raster_mod(font)
	local size = font[2]
	if not RASTER_DIMS[name] then
		warn("unknown raster font '%s', falling back to %s at size %d", font[1], RASTER_FALLBACK, RASTER_FALLBACK_SIZE)
		name = RASTER_FALLBACK
		size = RASTER_FALLBACK_SIZE
	end
	local dims = RASTER_DIMS[name]
	local glyph_h = size
	local glyph_w = math.floor(size * dims[1] / dims[2])
	return name, glyph_w, glyph_h
end

-- For font chains, use the first font's size for estimates
local function font_size(font)
	if is_font_chain(font) then return font[1][2] end
	return font[2]
end

local function char_width_est(font)
	if is_raster(font) then
		local _, glyph_w, _ = resolve_raster_font(font)
		return glyph_w
	else
		return math.max(1, math.floor(font_size(font) * 0.55))
	end
end

-- Count UTF-8 characters (not bytes)
local function utf8_charcount(s)
	local count = 0
	local i = 1
	while i <= #s do
		local b = s:byte(i)
		if b < 0x80 then i = i + 1
		elseif b < 0xE0 then i = i + 2
		elseif b < 0xF0 then i = i + 3
		else i = i + 4
		end
		count = count + 1
	end
	return count
end

-- Estimate pixel width of a UTF-8 string
local function text_width_est(s, font)
	local cw = char_width_est(font)
	local fs = font_size(font)
	local icon_w = math.max(1, math.floor(fs * 0.9))  -- FA icons are roughly square
	local w = 0
	local i = 1
	while i <= #s do
		local b = s:byte(i)
		if b < 0x80 then
			w = w + cw
			i = i + 1
		elseif b < 0xE0 then
			w = w + icon_w
			i = i + 2
		elseif b < 0xF0 then
			w = w + icon_w
			i = i + 3
		else
			w = w + icon_w
			i = i + 4
		end
	end
	return w
end

local function cell_height_est(font)
	if is_raster(font) then
		local _, _, glyph_h = resolve_raster_font(font)
		return glyph_h
	else
		return font_size(font)
	end
end

-------------------------------------------------------------------------------
-- 3. Default and focused styles from style.normal / style.focused
-------------------------------------------------------------------------------
local function make_border(b)
	b = b or {}
	return {
		width = b.width or 0,
		color = b.color or { 255, 255, 255 },
		side = b.side,
	}
end

if not style.normal.font then
	io.stderr:write("ERROR: style.normal.font is nil (undefined font?)\n")
	os.exit(1)
end
local default_style = {
	font = style.normal.font,
	fg = style.normal.fg,
	bg = style.normal.bg,
	pad = style.normal.pad or 0,
	pad_left = style.normal.pad_left,
	pad_top = style.normal.pad_top,
	pad_right = style.normal.pad_right,
	pad_bottom = style.normal.pad_bottom,
	margin = style.normal.margin or 0,
	margin_left = style.normal.margin_left,
	margin_top = style.normal.margin_top,
	margin_right = style.normal.margin_right,
	margin_bottom = style.normal.margin_bottom,
	border = make_border(style.normal.border),
}

local focused_ovr = style.focused or {}

local function copy_style(s)
	return {
		font = s.font,
		fg = { s.fg[1], s.fg[2], s.fg[3] },
		bg = { s.bg[1], s.bg[2], s.bg[3] },
		pad = s.pad,
		pad_left = s.pad_left,
		pad_top = s.pad_top,
		pad_right = s.pad_right,
		pad_bottom = s.pad_bottom,
		margin = s.margin,
		margin_left = s.margin_left,
		margin_top = s.margin_top,
		margin_right = s.margin_right,
		margin_bottom = s.margin_bottom,
		border = {
			width = s.border.width,
			color = { s.border.color[1], s.border.color[2], s.border.color[3] },
			side = s.border.side,
		},
	}
end

-- Valid keys per context (string keys only; integer keys are children)
local STYLE_KEYS = {
	font=1, fg=1, bg=1, pad=1, pad_left=1, pad_top=1, pad_right=1, pad_bottom=1,
	margin=1, margin_left=1, margin_top=1, margin_right=1, margin_bottom=1,
	border=1, align=1, valign=1,
}
local BORDER_KEYS = { width=1, color=1, side=1 }
local NODE_KEYS = {
	-- layout
	size=1, weight=1,
	-- behavior
	press=1, focusable=1, focus_index=1, lbl=1, render=1, align=1, fmt=1, adjust=1, focused=1, overload=1, active=1, active_id=1, icon=1, bidir=1, derived=1, derived_sep=1, derived_fn=1, ribbon=1, pointer_weight=1, font=1, fg=1, dim=1, dim_zeros=1, colors=1, chart_type=1,
	-- style (inline or via table)
	style=1, leaf_style=1,
	-- visual (when not using style= table)
	font=1, fg=1, bg=1, pad=1, pad_left=1, pad_top=1, pad_right=1, pad_bottom=1,
	margin=1, margin_left=1, margin_top=1, margin_right=1, margin_bottom=1,
	border=1,
	-- menu-level (on root container)
	menu_size=1, menu_align=1, menu_anchor=1,
}
local MENU_KEYS = {
	menu_size=1, menu_align=1, menu_anchor=1,
	-- containers also accept node keys
	style=1, leaf_style=1, border=1,
	font=1, fg=1, bg=1, pad=1, pad_left=1, pad_top=1, pad_right=1, pad_bottom=1,
	margin=1, margin_left=1, margin_top=1, margin_right=1, margin_bottom=1,
}

local function check_keys(tbl, valid, context)
	if type(tbl) ~= "table" then return end
	for k, _ in pairs(tbl) do
		if type(k) == "string" and not valid[k] then
			warn("unknown key '%s' in %s", k, context)
		end
	end
end

local function merge_style(base, node)
	local s = copy_style(base)
	if type(node) ~= "table" then
		return s
	end
	-- Style properties live under node.style if present; otherwise flat on node.
	local style_tbl = node.style
	node = style_tbl or node
	if style_tbl then
		check_keys(style_tbl, STYLE_KEYS, "style=")
	end
	if node.font then
		if type(node.font) ~= "table" or not node.font[1] then
			error("font must be a {path, size} table, got: " .. tostring(node.font), 2)
		end
		s.font = node.font
	end
	if node.fg then
		s.fg = { node.fg[1], node.fg[2], node.fg[3] }
	end
	if node.bg then
		s.bg = { node.bg[1], node.bg[2], node.bg[3] }
	end
	if node.pad then
		s.pad = node.pad
		s.pad_left = nil
		s.pad_top = nil
		s.pad_right = nil
		s.pad_bottom = nil
	end
	if node.pad_left then
		s.pad_left = node.pad_left
	end
	if node.pad_top then
		s.pad_top = node.pad_top
	end
	if node.pad_right then
		s.pad_right = node.pad_right
	end
	if node.pad_bottom then
		s.pad_bottom = node.pad_bottom
	end
	if node.margin then
		s.margin = node.margin
		s.margin_left = nil
		s.margin_top = nil
		s.margin_right = nil
		s.margin_bottom = nil
	end
	if node.margin_left then
		s.margin_left = node.margin_left
	end
	if node.margin_top then
		s.margin_top = node.margin_top
	end
	if node.margin_right then
		s.margin_right = node.margin_right
	end
	if node.margin_bottom then
		s.margin_bottom = node.margin_bottom
	end
	if node.border then
		check_keys(node.border, BORDER_KEYS, "border=")
		if node.border.width ~= nil then
			s.border.width = node.border.width
		end
		if node.border.color then
			s.border.color = { node.border.color[1], node.border.color[2], node.border.color[3] }
		end
		if node.border.side then
			s.border.side = node.border.side
		end
		if s.border.width == 0 and s.border.side then
			warn("border has side='%s' but width=0 (no pixels drawn)", s.border.side)
		end
	end
	if node.valign then
		s.valign = node.valign
	end
	return s
end

local function eff_margin(s, side)
	if side == "left" then return s.margin_left or s.margin end
	if side == "top" then return s.margin_top or s.margin end
	if side == "right" then return s.margin_right or s.margin end
	if side == "bottom" then return s.margin_bottom or s.margin end
	return s.margin
end

local function eff_pad(s, side)
	if side == "left" then
		return s.pad_left or s.pad
	end
	if side == "top" then
		return s.pad_top or s.pad
	end
	if side == "right" then
		return s.pad_right or s.pad
	end
	if side == "bottom" then
		return s.pad_bottom or s.pad
	end
end

-- Border inset for a given side: the border eats into usable space.
-- A full border (no side restriction) affects all four sides.
local function border_inset(s, side)
	if s.border.width == 0 then
		return 0
	end
	if s.border.side == nil then
		return s.border.width
	end
	if type(s.border.side) == "table" then
		for _, ss in ipairs(s.border.side) do
			if ss == side then return s.border.width end
		end
		return 0
	end
	if s.border.side == side then
		return s.border.width
	end
	return 0
end

-------------------------------------------------------------------------------
-- 4. Atlas registry
-------------------------------------------------------------------------------
local atlases = {} -- ordered list
local atlas_map = {} -- key string -> record

local function atlas_key(font, fg, bg)
	if is_raster(font) then
		local name, gw, gh = resolve_raster_font(font)
		return ("R:%s:%d:%d:%d:%d:%d:%d:%d:%d"):format(name, gw, gh, fg[1], fg[2], fg[3], bg[1], bg[2], bg[3])
	else
		if is_font_chain(font) then
			local parts = {}
			for _, f in ipairs(font) do parts[#parts+1] = f[1] .. ":" .. f[2] end
			return ("T:%s:%d:%d:%d:%d:%d:%d"):format(table.concat(parts, "+"), fg[1], fg[2], fg[3], bg[1], bg[2], bg[3])
		end
		return ("T:%s:%d:%d:%d:%d:%d:%d:%d"):format(font[1], font[2], fg[1], fg[2], fg[3], bg[1], bg[2], bg[3])
	end
end

local checked_fonts = {}
local function get_atlas(font, fg, bg)
	local k = atlas_key(font, fg, bg)
	if atlas_map[k] then
		return atlas_map[k]
	end
	-- Check font file exists (once per path)
	if not is_raster(font) then
		local paths = is_font_chain(font)
			and font  -- {{path,sz}, {path,sz}}
			or {font}  -- wrap single {"path",sz} into a list
		for _, fp in ipairs(paths) do
			local path = fp[1]
			if not checked_fonts[path] then
				checked_fonts[path] = true
				local f = io.open(path, "r")
				if f then f:close()
				else warn("font file not found: %s", path)
				end
			end
		end
	end
	local idx = #atlases + 1
	-- Collect extra code points from font spec
	local extra = font.extra or {}
	local rec = {
		key = k,
		font = font,
		extra = extra,
		fg = { fg[1], fg[2], fg[3] },
		bg = { bg[1], bg[2], bg[3] },
		idx = idx,
		varname = ("ATLAS_%d"):format(idx),
		fn_name = ("atlas_%d"):format(idx),
	}
	atlases[#atlases + 1] = rec
	atlas_map[k] = rec
	return rec
end

-------------------------------------------------------------------------------
-- 5. Layout pass
-------------------------------------------------------------------------------
local function is_container(node)
	return type(node) == "table" and (node[1] == "row" or node[1] == "col")
end

local function get_press(node)
	if type(node) ~= "table" then
		return nil
	end
	local p = node.press
	if not p then
		return nil
	end
	if type(p) == "string" then
		-- old syntax: warn and wrap
		warn('old press syntax press=%q, use press={"%s"}', p, p)
		return { p }
	end
	return p
end

local current_menu_name  -- set before each layout_node call
local function layout_node(node, x, y, w, h, ops, leaf_style)
	-- Validate keys on table nodes
	if type(node) == "table" then
		check_keys(node, NODE_KEYS, "node")
		if node.leaf_style then
			check_keys(node.leaf_style, STYLE_KEYS, "leaf_style=")
		end
	end

	-- Style: default_style → leaf_style (inherited) → node's own style.
	-- leaf_style propagates from ancestor containers to all descendant leaves.
	local s
	if leaf_style and not is_container(node) then
		s = merge_style(merge_style(default_style, leaf_style), node)
	else
		s = merge_style(default_style, node)
	end

	-- Apply margin (containers only use margin when explicitly set on node)
	local ml, mt, mr, mb = eff_margin(s,"left"), eff_margin(s,"top"), eff_margin(s,"right"), eff_margin(s,"bottom")
	if is_container(node) then
		local src = type(node) == "table" and (node.style or node) or nil
		if not (src and (src.margin or src.margin_left or src.margin_top or src.margin_right or src.margin_bottom)) then
			ml, mt, mr, mb = 0, 0, 0, 0
		end
	end
	x = x + ml
	y = y + mt
	w = w - ml - mr
	h = h - mt - mb
	if w < 0 or h < 0 then
		warn("margin exceeds available size, cell clipped to zero")
		w = math.max(0, w)
		h = math.max(0, h)
	end

	if is_container(node) then
		local dir = node[1]

		-- Combine inherited leaf_style with this container's own leaf_style
		local child_leaf_style = leaf_style
		if node.leaf_style then
			if child_leaf_style then
				-- Shallow merge: child overrides parent
				child_leaf_style = {}
				for k, v in pairs(leaf_style) do child_leaf_style[k] = v end
				for k, v in pairs(node.leaf_style) do child_leaf_style[k] = v end
			else
				child_leaf_style = node.leaf_style
			end
		end

		-- Background fill for container (only when bg explicitly set on this node)
		local src_bg = type(node) == "table" and (node.style or node) or nil
		if src_bg and src_bg.bg then
			ops[#ops + 1] = {
				kind = "fill",
				x = x, y = y, w = w, h = h,
				color = s.bg,
			}
		end

		-- Collect integer-keyed children (skip [1] which is "row"/"col")
		local children = {}
		for i = 2, #node do
			children[#children + 1] = node[i]
		end

		-- Containers only use border/padding/margin from default_style when
		-- explicitly set on THIS node; defaults only apply to leaves.
		local has_border = type(node) == "table" and node.border ~= nil
		local cbs = has_border and s
			or { border = { width = 0, color = s.border.color, side = s.border.side } }

		-- Container padding: only when explicitly set on node (not from default_style)
		local src = type(node) == "table" and (node.style or node) or nil
		local has_pad = src and (src.pad or src.pad_left or src.pad_top or src.pad_right or src.pad_bottom)
		local function cpad(side)
			if not has_pad then return 0 end
			return eff_pad(s, side)
		end
		local pl = cpad("left") + border_inset(cbs, "left")
		local pt = cpad("top") + border_inset(cbs, "top")
		local pr = cpad("right") + border_inset(cbs, "right")
		local pb = cpad("bottom") + border_inset(cbs, "bottom")
		local ix = x + pl
		local iy = y + pt
		local iw = w - pl - pr
		local ih = h - pt - pb

		-- Divide space: fixed-size children first, then weighted share the rest.
		-- For "col" containers, child.size means height in pixels.
		-- For "row" containers, child.size means width in pixels.
		local total_sz = (dir == "col") and ih or iw
		local total_fixed = 0
		local total_weight = 0
		local n_weighted = 0
		for _, ch in ipairs(children) do
			if type(ch) == "table" and ch.size then
				total_fixed = total_fixed + ch.size
			else
				total_weight = total_weight + (type(ch) == "table" and ch.weight or 1)
				n_weighted = n_weighted + 1
			end
		end
		if total_weight == 0 then
			total_weight = 1
		end
		local weighted_space = math.max(0, total_sz - total_fixed)

		local pos = (dir == "col") and iy or ix
		local used_weighted = 0
		local weighted_seen = 0
		for _, ch in ipairs(children) do
			local sz
			if type(ch) == "table" and ch.size then
				sz = ch.size
			else
				weighted_seen = weighted_seen + 1
				local weight = type(ch) == "table" and ch.weight or 1
				if weighted_seen == n_weighted then
					sz = weighted_space - used_weighted
				else
					sz = math.floor(weighted_space * weight / total_weight)
					used_weighted = used_weighted + sz
				end
			end
			if dir == "col" then
				layout_node(ch, ix, pos, iw, sz, ops, child_leaf_style)
			else
				layout_node(ch, pos, iy, sz, ih, ops, child_leaf_style)
			end
			pos = pos + sz
		end

		-- Border around container (only when explicitly set on this node)
		if cbs.border.width > 0 then
			ops[#ops + 1] = {
				kind = "border",
				x = x,
				y = y,
				w = w,
				h = h,
				thickness = cbs.border.width,
				color = cbs.border.color,
				side = cbs.border.side,
			}
		end

		-- Press zone: container itself is clickable
		local cpress = get_press(node)
		if cpress then
			ops[#ops + 1] = { kind = "press_zone", x = x, y = y, w = w, h = h, press = cpress }
		end

		-- Active styling on container: fill_rect + border, children redraw themselves
		if type(node) == "table" and node.active then
			ops[#ops + 1] = {
				kind = "container_active",
				x = x, y = y, w = w, h = h,
				bg = s.bg,
				active_style = node.active,
				active_id = node.active_id,
				normal_border = { width = cbs.border.width, color = cbs.border.color, side = cbs.border.side },
			}
		end
	else
		-- Leaf node
		local lbl, render, press, fmt, adjust, overload, active_style, active_id, bidir, derived, dim_zeros
		local text
		local icon_path

		if type(node) == "string" then
			text = node
		elseif type(node) == "table" then
			lbl = node.lbl
			render = node.render
			press = get_press(node)
			fmt = node.fmt
			adjust = node.adjust
			overload = node.overload
			active_style = node.active
			active_id = node.active_id
			icon_path = node.icon
			bidir = node.bidir
			dim_zeros = node.dim_zeros
			if node.derived then
				derived = { sources = node.derived, sep = node.derived_sep, fn_name = node.derived_fn }
			end
			if node.ribbon and type(node.ribbon) == "function" then
				render = "pointer slider"
			end
			if not lbl and not icon_path and type(node[1]) == "string" and node[1] ~= "row" and node[1] ~= "col" then
				text = node[1]
			end
		end

		-- Text alignment: "left" (default), "center", "right"
		-- Read from node.align, node.style.align, or leaf_style.align
		local align = "left"
		if type(node) == "table" then
			local a = node.align
				or (type(node.style) == "table" and node.style.align)
				or (leaf_style and leaf_style.align)
			if a then
				align = a
				if align ~= "left" and align ~= "center" and align ~= "right" then
					warn("unknown align=%q, using 'left'", align)
					align = "left"
				end
			end
		end

		-- Focusable flag: explicit override, or default to having a press handler
		local is_focusable
		local focus_index = type(node) == "table" and node.focus_index or nil
		if focus_index ~= nil then
			is_focusable = true
		elseif type(node) == "table" and node.focusable ~= nil then
			is_focusable = node.focusable
		else
			is_focusable = (press ~= nil)
		end

		local needs_atlas = (text or lbl) and render ~= "progress bar" and render ~= "chart"
		local atlas = needs_atlas and get_atlas(s.font, s.fg, s.bg) or nil
		-- Alternate styles for \x01-\x09 escape codes: each entry is {font=..., fg=...}
		-- Shorthand: bare color table = same font, different fg
		local alt_atlases = nil
		if atlas and type(node) == "table" then
			local alts = node.colors or (node.dim and {node.dim})
			if alts then
				if #alts > 9 then
					warn("colors= has %d entries but max is 9 (\\x01-\\x09) on label %q", #alts, lbl or text or "?")
				end
				alt_atlases = {}
				for i, alt in ipairs(alts) do
					if i > 9 then break end
					if type(alt) == "table" and alt.font then
						alt_atlases[#alt_atlases+1] = get_atlas(alt.font, alt.fg or s.fg, s.bg)
					else
						alt_atlases[#alt_atlases+1] = get_atlas(s.font, alt, s.bg)
					end
				end
			end
		end
		local ch_px = cell_height_est(s.font)
		local cw_px = char_width_est(s.font)
		-- Inset by border + padding so text is drawn inside both
		-- Use max of normal and focused border so text never overwrites focused border
		local function text_border(side)
			local nb = border_inset(s, side)
			if is_focusable then
				local fs = merge_style(s, focused_ovr)
				if type(node) == "table" and node.focused then
					fs = merge_style(fs, node.focused)
				end
				return math.max(nb, border_inset(fs, side))
			end
			return nb
		end
		local bx = text_border("left") + eff_pad(s, "left")
		local by = text_border("top") + eff_pad(s, "top")
		local bw = bx + text_border("right") + eff_pad(s, "right")
		local bh = by + text_border("bottom") + eff_pad(s, "bottom")
		local text_x = x + bx
		local inner_w = math.max(0, w - bw)
		local pad_chars = math.max(0, math.floor(inner_w / cw_px))
		-- Split text on \n; center the whole block vertically (unless valign="top")
		local line_gap = 2
		local lines = {}
		if text then
			for line in (text .. "\n"):gmatch("([^\n]*)\n") do
				lines[#lines + 1] = line
			end
		end
		local n_lines = math.max(1, #lines)
		local block_h = n_lines * ch_px + (n_lines - 1) * line_gap
		local valign = s.valign or "center"
		local text_y
		if valign == "top" then
			text_y = y + by
		elseif valign == "bottom" then
			text_y = y + h - bh + by - block_h
		else
			text_y = (y + by) + math.floor(((h - bh) - block_h) / 2)
		end
		local line_step = ch_px + line_gap
		-- Per-line x positions for static text alignment
		local line_xs = {}
		for i, line in ipairs(lines) do
			if align == "center" then
				local tw = text_width_est(line, s.font)
				line_xs[i] = text_x + math.floor((inner_w - tw) / 2)
			elseif align == "right" then
				local tw = text_width_est(line, s.font)
				line_xs[i] = text_x + (inner_w - tw)
			else
				line_xs[i] = text_x
			end
		end

		-- Compute focused style data for focusable cells
		local foc = nil
		if is_focusable then
			local fs = merge_style(s, focused_ovr)
			if type(node) == "table" and node.focused then
				fs = merge_style(fs, node.focused)
			end
			local fch_px = cell_height_est(fs.font)
			local fcw_px = char_width_est(fs.font)
			local fbx = border_inset(fs, "left") + eff_pad(fs, "left")
			local fby = border_inset(fs, "top") + eff_pad(fs, "top")
			local fbw = fbx + border_inset(fs, "right") + eff_pad(fs, "right")
			local fbh = fby + border_inset(fs, "bottom") + eff_pad(fs, "bottom")
			local fn_lines = math.max(1, #lines)
			local fblock_h = fn_lines * fch_px + (fn_lines - 1) * line_gap
			local f_text_x = x + fbx
			local f_inner_w = math.max(0, w - fbw)
			local f_line_xs = {}
			for i, line in ipairs(lines) do
				if align == "center" then
					local tw = text_width_est(line, fs.font)
					f_line_xs[i] = f_text_x + math.floor((f_inner_w - tw) / 2)
				elseif align == "right" then
					local tw = text_width_est(line, fs.font)
					f_line_xs[i] = f_text_x + (f_inner_w - tw)
				else
					f_line_xs[i] = f_text_x
				end
			end
			foc = {
				atlas = get_atlas(fs.font, fs.fg, fs.bg),
				text_x = f_text_x,
				inner_w = f_inner_w,
				align = align,
				line_xs = f_line_xs,
				text_y = valign == "top" and (y + fby)
					or valign == "bottom" and (y + h - fbh + fby - fblock_h)
					or (y + fby) + math.floor(((h - fbh) - fblock_h) / 2),
				line_step = fch_px + line_gap,
				pad_chars = math.max(0, math.floor(f_inner_w / fcw_px)),
				border = { width = fs.border.width, color = fs.border.color, side = fs.border.side },
				bg = fs.bg,
			}
		end

		-- Bounds checks: warn if text would exceed screen (skip zero-size cells)
		if w > 0 and h > 0 then
			local label = (text or lbl or ""):gsub("\n", "\\n")
			local text_bottom = text_y + block_h
			local text_right = text_x + pad_chars * cw_px
			if text_bottom > screen.height then
				warn("text bottom %d exceeds screen height %d: %q in %s",
					text_bottom, screen.height, label, current_menu_name or "?")
			end
			if text_right > screen.width then
				warn("text right %d exceeds screen width %d: %q in %s",
					text_right, screen.width, label, current_menu_name or "?")
			end
		end

		if icon_path then
			-- icon_path is an SVG file; render to .alpha at the target height
			local target_h = h - bh
			if target_h < 1 then target_h = 1 end
			local svg_base = icon_path:match("(.+)%.svg$")
			if not svg_base then
				io.stderr:write(("ERROR: icon must be an .svg file: %s\n"):format(icon_path))
				os.exit(1)
			end
			local alpha_path = svg_base .. "_" .. target_h .. ".alpha"
			-- Render if .alpha is missing or older than .svg
			local need_render = true
			local af = io.open(alpha_path, "rb")
			if af then
				af:close()
				-- Check mtimes: re-render if svg is newer
				local svg_attr = io.popen("stat -c %Y " .. icon_path):read("*a")
				local alpha_attr = io.popen("stat -c %Y " .. alpha_path):read("*a")
				if svg_attr and alpha_attr and tonumber(alpha_attr) >= tonumber(svg_attr) then
					need_render = false
				end
			end
			if need_render then
				local script_dir = debug.getinfo(1, "S").source:match("@?(.*/)")
				local cmd = ("python3 %srender_icon.py %s %d %s 1>&2"):format(
					script_dir, icon_path, target_h, alpha_path)
				local ok = os.execute(cmd)
				if not ok then
					io.stderr:write(("ERROR: failed to render icon: %s\n"):format(icon_path))
					os.exit(1)
				end
			end
			-- Read .alpha file: 4-byte header (u16 LE width, u16 LE height) + raw alpha
			local f = io.open(alpha_path, "rb")
			if not f then
				io.stderr:write(("ERROR: icon alpha file not found: %s\n"):format(alpha_path))
				os.exit(1)
			end
			local hdr = f:read(4)
			if not hdr or #hdr < 4 then
				f:close()
				io.stderr:write(("ERROR: icon alpha file is empty or corrupt: %s\n"):format(alpha_path))
				os.exit(1)
			end
			local iw = hdr:byte(1) + hdr:byte(2) * 256
			local ih = hdr:byte(3) + hdr:byte(4) * 256
			f:close()
			-- Center icon in cell
			local ix = x + bx + math.floor((inner_w - iw) / 2)
			local iy = y + by + math.floor(((h - bh) - ih) / 2)
			-- Resolve absolute path for include_bytes!
			local abs_path = io.popen("realpath " .. alpha_path):read("*l")
			ops[#ops + 1] = {
				kind = "icon",
				x = x, y = y, w = w, h = h,
				ix = ix, iy = iy, iw = iw, ih = ih,
				abs_icon_path = abs_path,
				fg = s.fg, bg = s.bg,
				press = press,
				is_focusable = is_focusable,
				focus_index = focus_index,
				foc = foc,
				normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
			}
		elseif lbl then
			if render == "progress bar" then
				-- For progress bars, use the max of normal and focused border
				-- so the fill area never overwrites the focused border
				local function prog_border(side)
					local nb = border_inset(s, side)
					if is_focusable then
						local fs = merge_style(s, focused_ovr)
						if type(node) == "table" and node.focused then
							fs = merge_style(fs, node.focused)
						end
						return math.max(nb, border_inset(fs, side))
					end
					return nb
				end
				local inset_l = eff_pad(s, "left") + prog_border("left")
				local inset_t = eff_pad(s, "top") + prog_border("top")
				local inset_r = eff_pad(s, "right") + prog_border("right")
				local inset_b = eff_pad(s, "bottom") + prog_border("bottom")
				ops[#ops + 1] = {
					kind = "progress",
					x = x, y = y, w = w, h = h,
					px = x + inset_l,
					py = y + inset_t,
					pw = math.max(0, w - inset_l - inset_r),
					ph = math.max(0, h - inset_t - inset_b),
					lbl = lbl,
					fg = s.fg,
					bg = s.bg,
					cell_bg = s.bg,
					adjust = adjust,
					overload = overload,
					bidir = bidir,
					is_focusable = is_focusable,
					focus_index = focus_index,
					foc = foc,
					normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
				}
			elseif render == "chart" then
				local inset_l = eff_pad(s, "left") + border_inset(s, "left")
				local inset_t = eff_pad(s, "top") + border_inset(s, "top")
				local inset_r = eff_pad(s, "right") + border_inset(s, "right")
				local inset_b = eff_pad(s, "bottom") + border_inset(s, "bottom")
				local cw = math.max(0, w - inset_l - inset_r)
				local ch = math.max(0, h - inset_t - inset_b)
				local ct = node.chart_type or "bar"
				ops[#ops + 1] = {
					kind = "chart",
					x = x, y = y, w = w, h = h,
					cx = x + inset_l, cy = y + inset_t, cw = cw, ch = ch,
					lbl = lbl,
					fg = s.fg,
					bg = s.bg,
					chart_type = ct,
					press = press,
					is_focusable = is_focusable,
					focus_index = focus_index,
					foc = foc,
					normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
				}
			elseif render == "pointer slider" then
				local pw = node.pointer_weight or 0.5
				if pw < 0.0 or pw > 1.0 then
					warn("pointer_weight %.2f out of range [0.0, 1.0], clamping", pw)
					pw = math.max(0.0, math.min(1.0, pw))
				end
				local inset_l = eff_pad(s, "left") + border_inset(s, "left")
				local inset_t = eff_pad(s, "top") + border_inset(s, "top")
				local inset_r = eff_pad(s, "right") + border_inset(s, "right")
				local inset_b = eff_pad(s, "bottom") + border_inset(s, "bottom")
				local sw = math.max(0, w - inset_l - inset_r)
				local sh = math.max(0, h - inset_t - inset_b)
				-- Pre-compute ribbon colors by calling the Lua function for each x
				local ribbon_colors = {}
				local ribbon_fn = type(node) == "table" and node.ribbon
				for xi = 0, sw - 1 do
					local t = sw > 1 and xi / (sw - 1) or 0
					local c = ribbon_fn(t)
					ribbon_colors[#ribbon_colors+1] = c
				end
				ops[#ops + 1] = {
					kind = "slider",
					x = x, y = y, w = w, h = h,
					sx = x + inset_l, sy = y + inset_t, sw = sw, sh = sh,
					lbl = lbl,
					fg = s.fg,
					bg = s.bg,
					ribbon = ribbon_colors,
					pointer_weight = pw,
					is_focusable = is_focusable,
					focus_index = focus_index,
					foc = foc,
					normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
				}
			else
				ops[#ops + 1] = {
					kind = "dynamic",
					x = x,
					y = y,
					w = w,
					h = h,
					text_x = text_x,
					text_y = text_y,
					lbl = lbl,
					fmt = fmt,
					atlas = atlas,
					alt_atlases = alt_atlases,
					pad_chars = pad_chars,
					align = align,
					inner_w = inner_w,
					press = press,
					is_focusable = is_focusable,
					focus_index = focus_index,
					foc = foc,
					normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
					active_style = active_style,
					active_id = active_id or (active_style and text),
					derived = derived,
					dim_zeros = dim_zeros,
				}
			end
		elseif text then
			ops[#ops + 1] = {
				kind = "static",
				x = x,
				y = y,
				w = w,
				h = h,
				text_x = text_x,
				text_y = text_y,
				inner_w = inner_w,
				align = align,
				line_xs = line_xs,
				lines = lines,
				line_step = line_step,
				atlas = atlas,
				press = press,
				is_focusable = is_focusable,
				focus_index = focus_index,
				foc = foc,
				normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
				active_style = active_style,
				active_id = active_id or (active_style and text),
			}
		end

		-- Border around leaf
		if s.border.width > 0 then
			ops[#ops + 1] = {
				kind = "border",
				x = x,
				y = y,
				w = w,
				h = h,
				thickness = s.border.width,
				color = s.border.color,
				side = s.border.side,
			}
		end
	end
end

-- Compute layout for all menus
local menu_names = {}
for name in pairs(menus) do
	check_menu_name(name)
	menu_names[#menu_names + 1] = name
end
table.sort(menu_names)

local menu_ops = {}
for _, name in ipairs(menu_names) do
	local m = menus[name]
	local mw = screen.width
	local mh = screen.height
	local mx = 0
	local my = 0

	if m.menu_size then
		mw = m.menu_size[1]
		mh = m.menu_size[2]
		if m.menu_align then
			local ax = m.menu_align[1]
			local ay = m.menu_align[2]
			local anchor = m.menu_anchor or "center"
			if anchor == "center" then
				mx = math.floor(ax - mw / 2)
				my = math.floor(ay - mh / 2)
			elseif anchor == "top_left" then
				mx = math.floor(ax)
				my = math.floor(ay)
			else
				warn("unknown anchor '%s', using center", anchor)
				mx = math.floor(ax - mw / 2)
				my = math.floor(ay - mh / 2)
			end
		else
			mx = math.floor((screen.width - mw) / 2)
			my = math.floor((screen.height - mh) / 2)
		end
	end

	local ops = {}
	current_menu_name = name
	check_keys(m, MENU_KEYS, "menu " .. name)
	layout_node(m, mx, my, mw, mh, ops)
	menu_ops[name] = ops
end

-------------------------------------------------------------------------------
-- 6. Emit
-------------------------------------------------------------------------------
local out_dir = arg[1]
if not out_dir then
	io.stderr:write("ERROR: usage: lua layout.lua <output-dir> < input.lua\n")
	os.exit(1)
end
local files = {}
local current_file = "mod.rs"
files[current_file] = {}
-- Menu files: strip one level of 4-space indent (left over from the old
-- inline mod block).  mod.rs lines are emitted verbatim.
local strip_indent = false
local function e(fmt, ...)
	local line
	if select("#", ...) > 0 then
		line = fmt:format(...)
	else
		line = fmt
	end
	if strip_indent then
		line = line:gsub("^    ", "")
	end
	local t = files[current_file]
	t[#t + 1] = line
end
local function switch_file(name)
	current_file = name
	if not files[name] then files[name] = {} end
	strip_indent = name ~= "mod.rs"
end

-- Header: sha256 of transpiler script + input, for reproducibility checks
local function sha256_file(path)
	local f = io.popen("sha256sum " .. path .. " 2>/dev/null || shasum -a 256 " .. path)
	if not f then
		return "unavailable"
	end
	local line = f:read("*l")
	f:close()
	return line and line:match("^(%x+)") or "unavailable"
end
local function sha256_str(s)
	local tmp = os.tmpname()
	local tf = io.open(tmp, "w")
	if not tf then
		return "unavailable"
	end
	tf:write(s)
	tf:close()
	local f = io.popen("sha256sum " .. tmp .. " 2>/dev/null || shasum -a 256 " .. tmp)
	if not f then
		os.remove(tmp)
		return "unavailable"
	end
	local line = f:read("*l")
	f:close()
	os.remove(tmp)
	return line and line:match("^(%x+)") or "unavailable"
end
local transpiler_path = arg and arg[0] or "scripts/layout.lua"
local transpiler_hash = sha256_file(transpiler_path)
local input_hash = sha256_str(src)

e("// auto-generated — do not edit")
e("// Generated by scripts/layout.lua — do not edit.")
e("// Re-run the transpiler to update:")
e("//   lua scripts/layout.lua < examples/ui.lua > examples/app/ui.rs")
e("// transpiler: %s", transpiler_hash)
e("// input:      %s", input_hash)
e("")

-- Propagate active styling from container_active to static and dynamic children
for _, name in ipairs(menu_names) do
	local containers = {}
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "container_active" then
			containers[#containers+1] = op
		end
	end
	for _, op in ipairs(menu_ops[name]) do
		if (op.kind == "dynamic" or op.kind == "static") and not op.active_style then
			for _, c in ipairs(containers) do
				if op.x >= c.x and op.y >= c.y
				   and op.x + op.w <= c.x + c.w
				   and op.y + op.h <= c.y + c.h then
					op.active_style = c.active_style
					break
				end
			end
		end
	end
end

-- Pre-create active atlases (must happen before atlas emission)
for _, name in ipairs(menu_names) do
	for _, op in ipairs(menu_ops[name]) do
		if (op.kind == "static" or op.kind == "dynamic") and op.active_style and (op.active_style.bg or op.active_style.fg) and op.atlas then
			local act_fg = op.active_style.fg or op.atlas.fg
			local act_bg = op.active_style.bg or op.atlas.bg
			op.active_atlas = get_atlas(op.atlas.font, act_fg, act_bg)
		end
	end
end

-- Imports
local need_raster = false
local need_ttf = false
local need_renderer = false
for _, a in ipairs(atlases) do
	if is_raster(a.font) then
		need_raster = true
	else
		need_ttf = true
	end
end
for _, ops in pairs(menu_ops) do
	for _, op in ipairs(ops) do
		if op.kind == "dynamic" and op.align ~= "left" then
			need_renderer = true
			break
		end
	end
end

-- Check if any menu uses icons
local need_icon = false
for _, name in ipairs(menu_names) do
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "icon" then need_icon = true; break end
	end
	if need_icon then break end
end
local extra_imports = ""
if need_renderer then extra_imports = extra_imports .. ", Renderer" end
if need_icon then extra_imports = extra_imports .. ", Icon" end
e("use super::Pixel;")
e("use imgrids::{Backend, InputEvent%s};", extra_imports)
if need_raster then
	e("use imgrids::raster::RasterAtlas;")
	e("type RasterAtlasP = RasterAtlas<Pixel>;")
end
if need_ttf then
	e("use imgrids::prebaked::PrebakedAtlas;")
	e("type PrebakedAtlasP = PrebakedAtlas<Pixel>;")
end
if #atlases > 0 then
	e("use std::sync::OnceLock;")
end

-- Check if any menu needs HashSet (for active ops)
local any_active = false

e("")

-- Generate pre-baked font atlas data via Python script
local ttf_atlases = {}
for _, a in ipairs(atlases) do
	if not is_raster(a.font) then
		local spec = { id = a.varname:lower(), extra = a.extra }
		if is_font_chain(a.font) then
			spec.fonts = {}
			for _, f in ipairs(a.font) do
				spec.fonts[#spec.fonts+1] = {f[1], f[2]}
			end
		else
			spec.fonts = {{a.font[1], a.font[2]}}
		end
		ttf_atlases[#ttf_atlases+1] = { spec = spec, atlas = a }
	end
end

if #ttf_atlases > 0 then
	-- Build JSON spec for the Python script
	local json_parts = {}
	for _, ta in ipairs(ttf_atlases) do
		local s = ta.spec
		local font_parts = {}
		for _, f in ipairs(s.fonts) do
			font_parts[#font_parts+1] = ('["%s", %d]'):format(f[1], f[2])
		end
		local extra_parts = {}
		for _, cp in ipairs(s.extra) do
			extra_parts[#extra_parts+1] = tostring(cp)
		end
		json_parts[#json_parts+1] = ('{"id": "%s", "fonts": [%s], "extra": [%s]}'):format(
			s.id, table.concat(font_parts, ", "), table.concat(extra_parts, ", "))
	end
	local json_str = "[" .. table.concat(json_parts, ", ") .. "]"

	-- Find the Python script relative to this script
	local script_dir = debug.getinfo(1, "S").source:match("@?(.*)/") or "."
	local py_script = script_dir .. "/gen_font_atlas.py"
	-- Output binary files to a cache directory alongside the generated ui.rs
	-- Use current working directory + target/font_cache
	local cwd = io.popen("pwd"):read("*l")
	local cache_dir = cwd .. "/target/font_cache"
	local py_cmd = ("echo '%s' | python3 '%s' '%s'"):format(json_str, py_script, cache_dir)
	local handle = io.popen(py_cmd, "r")
	local prebaked_src = handle:read("*a")
	local ok = handle:close()
	if not ok then
		io.stderr:write("ERROR: gen_font_atlas.py failed\n")
		os.exit(1)
	end

	-- Emit the prebaked data as a module
	e("#[allow(clippy::all)]")
	e("mod prebaked_fonts {")
	for line in prebaked_src:gmatch("[^\n]+") do
		if line:match("^%s*$") then
			e("")
		else
			e("    %s", line)
		end
	end
	e("}")
	e("")
end

-- Atlas statics and getters
for _, a in ipairs(atlases) do
	if is_raster(a.font) then
		local name, gw, gh = resolve_raster_font(a.font)
		e("static %s: OnceLock<RasterAtlasP> = OnceLock::new();", a.varname)
		e("fn %s() -> &'static RasterAtlasP {", a.fn_name)
		e("    %s.get_or_init(|| RasterAtlasP::new(", a.varname)
		e("        &imgrids::fonts::%s::FONT, %d, %d, %s, %s,", name, gw, gh, rgb_lit(a.fg), rgb_lit(a.bg))
		e("    ))")
		e("}")
	else
		local vid = a.varname
		e("static %s: OnceLock<PrebakedAtlasP> = OnceLock::new();", a.varname)
		e("fn %s() -> &'static PrebakedAtlasP {", a.fn_name)
		e("    %s.get_or_init(|| PrebakedAtlasP::from_alpha(", a.varname)
		e("        prebaked_fonts::%s_CELL_H,", vid)
		e("        &prebaked_fonts::%s_ASCII_ADV,", vid)
		e("        &prebaked_fonts::%s_ASCII_OFF,", vid)
		e("        prebaked_fonts::%s_EXT,", vid)
		e("        prebaked_fonts::%s_ALPHA,", vid)
		e("        %s, %s,", rgb_lit(a.fg), rgb_lit(a.bg))
		e("    ))")
		e("}")
	end
	e("")
end

-- Screen dimensions
e("pub const SCR_W: usize = %d;", screen.width)
e("pub const SCR_H: usize = %d;", screen.height)
e("")

-- Menu enum
e("#[derive(Clone, Copy, PartialEq)]")
e("pub enum Menu {")
for _, name in ipairs(menu_names) do
	e("    %s,", name)
end
e("}")
e("")

-------------------------------------------------------------------------------
-- 6a. Per-menu index assignment (locally numbered per menu)
-------------------------------------------------------------------------------
local menu_prog_ops = {}
local menu_dyn_ops = {}
local menu_slider_ops = {}
local menu_chart_ops = {}
local menu_active_ops = {}
local menu_derived_ops = {}

for _, name in ipairs(menu_names) do
	menu_prog_ops[name] = {}
	menu_dyn_ops[name] = {}
	menu_slider_ops[name] = {}
	menu_chart_ops[name] = {}
	menu_active_ops[name] = {}
	menu_derived_ops[name] = {}

	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "progress" then
			local t = menu_prog_ops[name]
			t[#t+1] = op
			op.prog_idx = #t
		end
		if op.kind == "slider" then
			local t = menu_slider_ops[name]
			t[#t+1] = op
			op.slider_idx = #t
		end
		if op.kind == "chart" then
			local t = menu_chart_ops[name]
			t[#t+1] = op
			op.chart_idx = #t
		end
		if op.kind == "dynamic" then
			local t = menu_dyn_ops[name]
			t[#t+1] = op
			op.dyn_idx = #t
		end
	end
end

-- Active index assignment (per menu)
for _, name in ipairs(menu_names) do
	-- First assign indices to container_active ops
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "container_active" and op.active_style then
			local t = menu_active_ops[name]
			t[#t+1] = op
			op.active_idx = #t
			op.active_menu = name
		end
	end
	-- Then propagate active_idx to children
	local containers = {}
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "container_active" then
			containers[#containers+1] = op
		end
	end
	for _, op in ipairs(menu_ops[name]) do
		if (op.kind == "dynamic" or op.kind == "static") and op.active_style and not op.active_idx then
			-- Check if this is a propagated child
			for _, c in ipairs(containers) do
				if op.x >= c.x and op.y >= c.y
				   and op.x + op.w <= c.x + c.w
				   and op.y + op.h <= c.y + c.h then
					op.active_idx = c.active_idx
					break
				end
			end
			-- Non-propagated leaf with its own active_style
			if not op.active_idx then
				local t = menu_active_ops[name]
				t[#t+1] = op
				op.active_idx = #t
				op.active_menu = name
			end
		end
	end

	if #menu_active_ops[name] > 0 then
		any_active = true
	end
end

-- Collect set_stay buttons for auto-active (per menu)
local auto_active = {}
for _, name in ipairs(menu_names) do
	auto_active[name] = {}
	for _, op in ipairs(menu_ops[name]) do
		if op.press and (op.press[1] == "set_stay" or op.press[1] == "set_param" or op.press[1] == "set_nav" or op.press[1] == "set_ref_source") and op.active_idx then
			local param = op.press[2]
			local value = op.press[3]
			if param and value then
				if not auto_active[name][param] then
					auto_active[name][param] = {}
				end
				auto_active[name][param][#auto_active[name][param]+1] = {
					value = value, idx = op.active_idx
				}
			end
		end
	end
end

-- Derived label ops (per menu)
for _, name in ipairs(menu_names) do
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "dynamic" and op.derived then
			local t = menu_derived_ops[name]
			t[#t+1] = op
			op.derived_idx = #t
		end
	end
end

-- Collect callbacks: map fn_name -> max_nargs
local callbacks = {}
local callback_list = {}
for _, name in ipairs(menu_names) do
	for _, op in ipairs(menu_ops[name]) do
		if op.press then
			local fn_name = op.press[1]
			local nargs = #op.press - 1
			if callbacks[fn_name] == nil then
				callbacks[fn_name] = nargs
				callback_list[#callback_list + 1] = fn_name
			elseif nargs > callbacks[fn_name] then
				callbacks[fn_name] = nargs
			end
		end
	end
end
table.sort(callback_list)

-- Callbacks trait
e("pub trait Callbacks {")
e("    fn quit(&mut self);")
for _, fn_name in ipairs(callback_list) do
	if callbacks[fn_name] > 0 then
		e("    fn %s(&mut self, args: &[&str]);", fn_name)
	else
		e("    fn %s(&mut self);", fn_name)
	end
end
e("}")
e("")

-- to_menu()
e("pub fn to_menu(name: &str) -> Option<Menu> {")
e("    match name {")
for _, name in ipairs(menu_names) do
	e("        %q => Some(Menu::%s),", name, name)
end
e("        _ => None,")
e("    }")
e("}")
e("")

-------------------------------------------------------------------------------
-- 6b. Helper functions
-------------------------------------------------------------------------------

-- Helper: emit a static text blit with pixel-perfect alignment
local function emit_static_blit(indent, atlas_fn, align, text_x, inner_w, y, line)
	if align == "center" then
		e("%s{ let tw = %s().text_width(%q);", indent, atlas_fn, line)
		e("%s  backend.blit(%s(), %d + %d_usize.saturating_sub(tw) / 2, %d, %q); }",
			indent, atlas_fn, text_x, inner_w, y, line)
	elseif align == "right" then
		e("%s{ let tw = %s().text_width(%q);", indent, atlas_fn, line)
		e("%s  backend.blit(%s(), %d + %d_usize.saturating_sub(tw), %d, %q); }",
			indent, atlas_fn, text_x, inner_w, y, line)
	else
		e("%sbackend.blit(%s(), %d, %d, %q);", indent, atlas_fn, text_x, y, line)
	end
end

-- Helper: collect focusable ops, assign focus indices.
local function get_focusable_ops(ops)
	local raw = {}
	for _, op in ipairs(ops) do
		if (op.kind == "static" or op.kind == "dynamic" or op.kind == "progress" or op.kind == "icon" or op.kind == "chart") and op.is_focusable then
			raw[#raw + 1] = op
		end
	end
	local explicit = {}
	for _, op in ipairs(raw) do
		if op.focus_index then explicit[op.focus_index] = true end
	end
	local next_auto = 0
	for _, op in ipairs(raw) do
		if not op.focus_index then
			while explicit[next_auto] do next_auto = next_auto + 1 end
			op.focus_index = next_auto
			next_auto = next_auto + 1
		end
	end
	table.sort(raw, function(a, b) return a.focus_index < b.focus_index end)
	return raw
end

-- Helper: emit blit code for a single dynamic label (using self.field access)
local function emit_dyn_blit(op, indent, val_expr)
	val_expr = val_expr or "val"
	local ch = cell_height_est(op.atlas.font)
	local bg = rgb_lit(op.atlas.bg)
	local atlas_fn = op.atlas.fn_name
	local dyn_end = ("self.dyn_%d_end"):format(op.dyn_idx)
	-- Dynamic nodes inside active containers: bg and atlas depend on active state
	if op.active_idx then
		local act_bg = op.active_style.bg and rgb_lit(op.active_style.bg) or bg
		local act_atlas = op.active_atlas and op.active_atlas.fn_name or atlas_fn
		e("%slet is_active = self.active.contains(&%d);", indent, op.active_idx)
		e("%slet bg = if is_active { %s } else { %s };", indent, act_bg, bg)
		if act_atlas ~= atlas_fn then
			e("%slet a = if is_active { %s() } else { %s() };", indent, act_atlas, atlas_fn)
		end
		bg = "bg"
	end
	if op.fmt then
		e("%slet %s = &crate::%s(%s);", indent, val_expr, op.fmt, val_expr)
	end

	-- Multi-color/font support: \x01-\x09 select alternate atlas, \x00 restores default
	if op.alt_atlases and #op.alt_atlases > 0 then
		local alt_fns = {}
		for _, a in ipairs(op.alt_atlases) do
			alt_fns[#alt_fns+1] = ("%s()"):format(a.fn_name)
		end
		e("%s{", indent)
		e("%s    let prev_end = %s;", indent, dyn_end)
		e("%s    let clear_w = if prev_end == usize::MAX { %d } else { prev_end.saturating_sub(%d) };",
			indent, op.inner_w, op.text_x)
		local clear_h = op.h - (op.text_y - op.y)
		e("%s    backend.fill_rect(%d, %d, clear_w, %d, %s);", indent, op.text_x, op.text_y, clear_h, bg)
		if op.align == "center" then
			e("%s    let tw: usize = %s.chars().filter(|&c| c > '\\x09' || c == '\\n').map(|c| if c == '\\n' { 0 } else { %s().char_width(c) }).sum();",
				indent, val_expr, atlas_fn)
			e("%s    let start_x = %d + (%d_usize.saturating_sub(tw)) / 2;",
				indent, op.text_x, op.inner_w)
		elseif op.align == "right" then
			e("%s    let tw: usize = %s.chars().filter(|&c| c > '\\x09' || c == '\\n').map(|c| if c == '\\n' { 0 } else { %s().char_width(c) }).sum();",
				indent, val_expr, atlas_fn)
			e("%s    let start_x = %d + %d_usize.saturating_sub(tw);",
				indent, op.text_x, op.inner_w)
		else
			e("%s    let start_x = %d_usize;", indent, op.text_x)
		end
		e("%s    let mut cx = start_x;", indent)
		e("%s    let alts: &[&dyn imgrids::Renderer<Pixel>] = &[%s];", indent, table.concat(alt_fns, ", "))
		e("%s    let mut cur: &dyn imgrids::Renderer<Pixel> = %s();", indent, atlas_fn)
		e("%s    let mut cy = %d_usize;", indent, op.text_y)
		e("%s    for c in %s.chars() {", indent, val_expr)
		e("%s        if c == '\\x00' { cur = %s(); continue; }", indent, atlas_fn)
		e("%s        if ('\\x01'..='\\x09').contains(&c) {", indent)
		e("%s            let idx = (c as usize) - 1;", indent)
		e("%s            if idx < alts.len() { cur = alts[idx]; }", indent)
		e("%s            continue;", indent)
		e("%s        }", indent)
		e("%s        if c == '\\n' { cy += %s().cell_height() + 2; cx = start_x; continue; }", indent, atlas_fn)
		e("%s        let s: String = c.to_string();", indent)
		e("%s        cx = backend.blit(cur, cx, cy, &s);", indent)
		e("%s    }", indent)
		e("%s    %s = cx;", indent, dyn_end)
		e("%s}", indent)
		return
	end

	-- For active nodes, 'a' is already set; for non-active, set it below
	local use_local_a = op.active_idx and op.active_atlas and op.active_atlas.fn_name ~= op.atlas.fn_name
	if op.align == "left" then
		if use_local_a then
			e("%slet end_x = backend.blit_clipped(a, %d, %d, val, %d);",
				indent, op.text_x, op.text_y, op.text_x + op.inner_w)
		else
			e("%slet end_x = backend.blit_clipped(%s(), %d, %d, val, %d);",
				indent, atlas_fn, op.text_x, op.text_y, op.text_x + op.inner_w)
		end
		e("%slet prev = %s; %s = end_x;", indent, dyn_end, dyn_end)
		e("%sif prev != usize::MAX && prev > end_x {", indent)
		e("%s    backend.fill_rect(end_x, %d, prev - end_x, %d, %s);", indent, op.text_y, ch, bg)
		e("%s}", indent)
	else
		-- Center/right: clear old area then blit at computed x
		if not use_local_a then
			e("%slet a = %s();", indent, atlas_fn)
		end
		e("%slet prev = %s;", indent, dyn_end)
		e("%slet clear_w = if prev == usize::MAX { %d } else { prev.saturating_sub(%d) };",
			indent, op.inner_w, op.text_x)
		e("%sbackend.fill_rect(%d, %d, clear_w, %d, %s);", indent, op.text_x, op.text_y, ch, bg)
		if op.align == "center" then
			e("%slet tw = a.text_width(val);", indent)
			e("%slet bx = %d + (%d_usize.saturating_sub(tw)) / 2;",
				indent, op.text_x, op.inner_w)
		else
			e("%slet tw = a.text_width(val);", indent)
			e("%slet bx = %d + %d_usize.saturating_sub(tw);",
				indent, op.text_x, op.inner_w)
		end
		e("%slet end_x = backend.blit(a, bx, %d, val);", indent, op.text_y)
		e("%s%s = end_x;", indent, dyn_end)
	end
end

-- Helper: emit border drawing lines
local function emit_border(indent, op_x, op_y, op_w, op_h, b)
	if b.width == 0 then
		return
	end
	local i = indent
	if b.side then
		local x2, y2, w2, h2, t, c = op_x, op_y, op_w, op_h, b.width, rgb_lit(b.color)
		local sides = type(b.side) == "table" and b.side or {b.side}
		for _, s in ipairs(sides) do
			if s == "top" then
				e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2, w2, t, c)
			elseif s == "bottom" then
				e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2 + h2 - t, w2, t, c)
			elseif s == "left" then
				e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2, t, h2, c)
			elseif s == "right" then
				e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2 + w2 - t, y2, t, h2, c)
			end
		end
	else
		local t, c = b.width, rgb_lit(b.color)
		e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, op_x, op_y, op_w, t, c)
		e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, op_x, op_y + op_h - t, op_w, t, c)
		e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, op_x, op_y, t, op_h, c)
		e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, op_x + op_w - t, op_y, t, op_h, c)
	end
end

-------------------------------------------------------------------------------
-- 6c. Per-menu modules
-------------------------------------------------------------------------------

for _, name in ipairs(menu_names) do
	local ops = menu_ops[name]
	local m_prog_ops = menu_prog_ops[name]
	local m_dyn_ops = menu_dyn_ops[name]
	local m_slider_ops = menu_slider_ops[name]
	local m_chart_ops = menu_chart_ops[name]
	local m_active_ops = menu_active_ops[name]
	local m_derived_ops = menu_derived_ops[name]
	local fops = get_focusable_ops(ops)

	local has_active = #m_active_ops > 0
	local has_focus = #fops > 0

	e("mod menu_%s;", name:lower())
	switch_file("menu_" .. name:lower() .. ".rs")
	e("use super::*;")
	if has_active then
		e("use std::collections::HashSet;")
	end
	e("")

	-- Slider ribbon constants
	for i, op in ipairs(m_slider_ops) do
		local parts = {}
		for _, c in ipairs(op.ribbon) do
			parts[#parts+1] = rgb_lit(c)
		end
		e("    const SLIDER_%d_RIBBON: &[Pixel] = &[%s];", i, table.concat(parts, ", "))
	end
	if #m_slider_ops > 0 then e("") end

	-- State struct
	e("    pub(super) struct State {")
	e("        pub(super) focused: Option<usize>,")
	e("        last_drawn_focus: Option<usize>,")
	if has_active then
		e("        pub(super) active: HashSet<usize>,")
		e("        last_drawn_active: HashSet<usize>,")
	end
	for i = 1, #m_prog_ops do
		e("        prog_%d_prev: usize,", i)
	end
	for i = 1, #m_dyn_ops do
		e("        dyn_%d_end: usize,", i)
	end
	for i = 1, #m_slider_ops do
		e("        slider_%d_prev: usize,", i)
	end
	for i = 1, #m_chart_ops do
		e("        chart_%d_data: Vec<f64>,", i)
		e("        chart_%d_dirty: bool,", i)
	end
	for _, op in ipairs(m_derived_ops) do
		for j = 1, #op.derived.sources do
			e("        derived_%d_%d: String,", op.derived_idx, j)
		end
	end
	e("    }")
	e("")

	-- State::new()
	e("    impl State {")
	e("    pub(super) fn new() -> Self {")
	e("        State {")
	e("            focused: None,")
	e("            last_drawn_focus: None,")
	if has_active then
		e("            active: HashSet::new(),")
		e("            last_drawn_active: HashSet::new(),")
	end
	for i = 1, #m_prog_ops do
		e("            prog_%d_prev: usize::MAX,", i)
	end
	for i = 1, #m_dyn_ops do
		e("            dyn_%d_end: usize::MAX,", i)
	end
	for i = 1, #m_slider_ops do
		e("            slider_%d_prev: usize::MAX,", i)
	end
	for i = 1, #m_chart_ops do
		e("            chart_%d_data: Vec::new(),", i)
		e("            chart_%d_dirty: true,", i)
	end
	-- Derived label defaults from params.txt
	for _, op in ipairs(m_derived_ops) do
		for j, src in ipairs(op.derived.sources) do
			local default = ""
			if type(params) == "table" then
				for _, p in ipairs(params) do
					if p.name == src and p.default ~= nil then
						if p.opts and type(p.default) == "number" then
							local idx = p.default + 1
							if p.opts[idx] then
								default = p.opts[idx]
							else
								default = tostring(p.default)
							end
						else
							default = tostring(p.default)
						end
						break
					end
				end
			end
			if default ~= "" then
				e("            derived_%d_%d: %q.to_owned(),", op.derived_idx, j, default)
			else
				e("            derived_%d_%d: String::new(),", op.derived_idx, j)
			end
		end
	end
	e("        }")
	e("    }")
	e("")

	-- State::reset()
	e("    pub(super) fn reset(&mut self) {")
	e("        self.focused = None;")
	e("        self.last_drawn_focus = None;")
	if has_active then
		e("        self.active.clear();")
		e("        self.last_drawn_active.clear();")
	end
	for i = 1, #m_prog_ops do
		e("        self.prog_%d_prev = usize::MAX;", i)
	end
	for i = 1, #m_dyn_ops do
		e("        self.dyn_%d_end = usize::MAX;", i)
	end
	for i = 1, #m_slider_ops do
		e("        self.slider_%d_prev = usize::MAX;", i)
	end
	for i = 1, #m_chart_ops do
		e("        self.chart_%d_dirty = true;", i)
	end
	e("    }")
	e("")

	-- State::draw()
	e("    pub(super) fn draw(&self, backend: &mut dyn Backend<Pixel>) {")
	e("        backend.fill_rect(0, 0, %d, %d, %s);", screen.width, screen.height, rgb_lit(default_style.bg))
	for _, op in ipairs(ops) do
		if op.kind == "icon" then
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
			e("        backend.blit_alpha(&Icon { x: %d, y: %d, w: %d, h: %d, alpha: &include_bytes!(%q)[4..] }, %s, %s);",
				op.ix, op.iy, op.iw, op.ih, op.abs_icon_path, rgb_lit(op.fg), rgb_lit(op.bg))
		elseif op.kind == "static" then
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.atlas.bg))
			for i, line in ipairs(op.lines) do
				local y = op.text_y + (i - 1) * op.line_step
				emit_static_blit("        ", op.atlas.fn_name, op.align,
					op.text_x, op.inner_w, y, line)
			end
		elseif op.kind == "dynamic" or op.kind == "progress" or op.kind == "slider" or op.kind == "chart" then
			local bg = op.atlas and op.atlas.bg or op.bg
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(bg))
		elseif op.kind == "fill" then
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.color))
		elseif op.kind == "border" then
			if op.side then
				local bx, by, bw, bh, t, c = op.x, op.y, op.w, op.h, op.thickness, rgb_lit(op.color)
				local sides = type(op.side) == "table" and op.side or {op.side}
				for _, s in ipairs(sides) do
					if s == "top" then
						e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by, bw, t, c)
					elseif s == "bottom" then
						e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by + bh - t, bw, t, c)
					elseif s == "left" then
						e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by, t, bh, c)
					elseif s == "right" then
						e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx + bw - t, by, t, bh, c)
					end
				end
			else
				local bx, by, bw, bh, t, c = op.x, op.y, op.w, op.h, op.thickness, rgb_lit(op.color)
				e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by, bw, t, c)
				e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by + bh - t, bw, t, c)
				e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx, by, t, bh, c)
				e("        backend.fill_rect(%d, %d, %d, %d, %s);", bx + bw - t, by, t, bh, c)
			end
		end
	end
	e("    }")
	e("")

	-- State::draw_focus()
	if has_focus then
		e("    fn draw_focus(&mut self, backend: &mut dyn Backend<Pixel>, prev: Option<usize>, focused: Option<usize>) {")
		for fi, op in ipairs(fops) do
			local idx = op.focus_index
			e("        if focused == Some(%d) && prev != Some(%d) {", idx, idx)
			if op.kind == "progress" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.foc.border)
				e("            self.prog_%d_prev = usize::MAX;", op.prog_idx)
			elseif op.kind == "chart" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.foc.border)
			elseif op.kind == "icon" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.foc.bg))
				e("            backend.blit_alpha(&Icon { x: %d, y: %d, w: %d, h: %d, alpha: &include_bytes!(%q)[4..] }, %s, %s);",
					op.ix, op.iy, op.iw, op.ih, op.abs_icon_path, rgb_lit(op.fg), rgb_lit(op.foc.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.foc.border)
			else
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.foc.bg))
				if op.kind == "static" then
					for i, line in ipairs(op.lines) do
						local y = op.foc.text_y + (i - 1) * op.foc.line_step
						emit_static_blit("            ", op.foc.atlas.fn_name, op.foc.align,
							op.foc.text_x, op.foc.inner_w, y, line)
					end
				end
				emit_border("            ", op.x, op.y, op.w, op.h, op.foc.border)
			end
			e("        }")
			e("        if prev == Some(%d) && focused != Some(%d) {", idx, idx)
			if op.kind == "progress" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.normal_border)
				e("            self.prog_%d_prev = usize::MAX;", op.prog_idx)
			elseif op.kind == "chart" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.normal_border)
			elseif op.kind == "icon" then
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.bg))
				e("            backend.blit_alpha(&Icon { x: %d, y: %d, w: %d, h: %d, alpha: &include_bytes!(%q)[4..] }, %s, %s);",
					op.ix, op.iy, op.iw, op.ih, op.abs_icon_path, rgb_lit(op.fg), rgb_lit(op.bg))
				emit_border("            ", op.x, op.y, op.w, op.h, op.normal_border)
			else
				e("            backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.atlas.bg))
				if op.kind == "static" then
					for i, line in ipairs(op.lines) do
						local y = op.text_y + (i - 1) * op.line_step
						emit_static_blit("            ", op.atlas.fn_name, op.align,
							op.text_x, op.inner_w, y, line)
					end
				end
				emit_border("            ", op.x, op.y, op.w, op.h, op.normal_border)
			end
			e("        }")
		end
		e("    }")
		e("")
	end

	-- State::update_events()
	local press_ops = {}
	for _, op in ipairs(ops) do
		if op.press then
			press_ops[#press_ops + 1] = op
		end
	end
	local has_press = #press_ops > 0

	local ev_param = (has_press or has_focus) and "events" or "_events"
	local st_param = has_press and "state" or "_state"
	local needs_press = false
	for _, op in ipairs(press_ops) do
		if op.press[1] == "select_digit" then needs_press = true; break end
	end
	local press_param = needs_press and "press" or "_press"
	e("    pub(super) fn update_events<C: Callbacks>(&mut self, %s: &[InputEvent], %s: &mut C, %s: &mut (usize, usize)) {", ev_param, st_param, press_param)

	if has_press or has_focus then
		e("        for ev in events {")
		e("            if let InputEvent::Press { x, y } = ev {")

		if has_focus then
			if #fops == 1 then
				local op = fops[1]
				local x_lo = op.x > 0 and ("*x >= %d && "):format(op.x) or ""
				local y_lo = op.y > 0 and ("*y >= %d && "):format(op.y) or ""
				e(
					"                self.focused = if %s*x < %d && %s*y < %d { Some(%d) } else { None };",
					x_lo,
					op.x + op.w,
					y_lo,
					op.y + op.h,
					op.focus_index
				)
			else
				local groups = {}
				local group_order = {}
				for _, op in ipairs(fops) do
					local idx = op.focus_index
					if not groups[idx] then
						groups[idx] = { x = op.x, y = op.y, x2 = op.x + op.w, y2 = op.y + op.h }
						group_order[#group_order + 1] = idx
					else
						local g = groups[idx]
						g.x  = math.min(g.x, op.x)
						g.y  = math.min(g.y, op.y)
						g.x2 = math.max(g.x2, op.x + op.w)
						g.y2 = math.max(g.y2, op.y + op.h)
					end
				end
				e("                let new_focus =")
				for gi, idx in ipairs(group_order) do
					local g = groups[idx]
					local x_lo = g.x > 0 and ("*x >= %d && "):format(g.x) or ""
					local y_lo = g.y > 0 and ("*y >= %d && "):format(g.y) or ""
					local prefix = gi == 1 and "                    if " or "                    else if "
					e("%s%s*x < %d && %s*y < %d { Some(%d) }", prefix, x_lo, g.x2, y_lo, g.y2, idx)
				end
				e("                    else { None };")
				e("                self.focused = new_focus;")
			end
		end

		if has_press then
			for _, op in ipairs(press_ops) do
				local fn_name = op.press[1]
				local x_lo = op.x > 0 and ("*x >= %d && "):format(op.x) or ""
				local y_lo = op.y > 0 and ("*y >= %d && "):format(op.y) or ""
				e("                if %s*x < %d && %s*y < %d {", x_lo, op.x + op.w, y_lo, op.y + op.h)
				if fn_name == "select_digit" then
					e("                    press.0 = *x;")
					e("                    press.1 = %d;", op.x + op.w)
				end
				if callbacks[fn_name] > 0 then
					local args = {}
					for i = 2, #op.press do
						args[#args + 1] = ("%q"):format(op.press[i])
					end
					e("                    state.%s(&[%s]);", fn_name, table.concat(args, ", "))
				else
					e("                    state.%s();", fn_name)
				end
				e("                }")
			end
		end

		e("            }")
		e("        }")
	end

	e("    }")
	e("")

	-- State::update_params()
	local dyn_ops = {}
	local prog_ops = {}
	local chart_ops = {}
	for _, op in ipairs(ops) do
		if op.kind == "dynamic" then
			dyn_ops[#dyn_ops + 1] = op
		end
		if op.kind == "progress" then
			prog_ops[#prog_ops + 1] = op
		end
		if op.kind == "chart" then
			chart_ops[#chart_ops + 1] = op
		end
	end

	local aa = auto_active[name]
	local aa_params = {}
	for param in pairs(aa) do aa_params[#aa_params+1] = param end
	table.sort(aa_params)
	local has_auto_active = #aa_params > 0
	local has_content = #dyn_ops + #prog_ops + #chart_ops + #fops + (has_auto_active and 1 or 0)
	local be_param = has_content > 0 and "backend" or "_backend"
	local chg_param = (#dyn_ops + #prog_ops + #chart_ops + (has_auto_active and 1 or 0)) > 0 and "changes" or "_changes"
	e("    pub(super) fn update_params(&mut self, %s: &mut dyn Backend<Pixel>, %s: &[(&str, &str)]) {", be_param, chg_param)

	-- Derived labels: watch constituent params and rebuild
	local derived_ops = {}
	for _, op in ipairs(dyn_ops) do
		if op.derived then derived_ops[#derived_ops+1] = op end
	end
	for _, op in ipairs(derived_ops) do
		local dyn_end = ("self.dyn_%d_end"):format(op.dyn_idx)
		e("        {")
		e("            let mut dirty = %s == usize::MAX;", dyn_end)
		e("            for &(n, v) in changes {")
		for j, src in ipairs(op.derived.sources) do
			e("                if n == %q { self.derived_%d_%d = v.to_owned(); dirty = true; }", src, op.derived_idx, j)
		end
		e("            }")
		e("            if dirty {")
		if op.derived.fn_name == "dots" then
			e("                let idx: usize = self.derived_%d_1.parse().unwrap_or(0);", op.derived_idx)
			e("                let val = \"\\u{00B7}\".repeat(idx + 1);")
		elseif op.derived.sep then
			local parts = {}
			for j, src in ipairs(op.derived.sources) do
				parts[#parts+1] = ("self.derived_%d_%d.clone()"):format(op.derived_idx, j)
			end
			e("                let val = [%s].join(%q);", table.concat(parts, ", "), op.derived.sep)
		end
		e("                let val: &str = &val;")
		emit_dyn_blit(op, "                ")
		e("            }")
		e("        }")
	end

	-- Filter out derived ops from normal dyn processing
	local normal_dyn_ops = {}
	for _, op in ipairs(dyn_ops) do
		if not op.derived then normal_dyn_ops[#normal_dyn_ops+1] = op end
	end

	if #normal_dyn_ops > 0 then
		local seen_lbl = {}
		local conds = {}
		for _, op in ipairs(normal_dyn_ops) do
			if not seen_lbl[op.lbl] then
				seen_lbl[op.lbl] = true
				conds[#conds+1] = ("n == %q"):format(op.lbl)
			end
		end
		e("        if changes.iter().any(|&(n, _)| %s) {", table.concat(conds, " || "))
		e("            for &(name, val) in changes {")
		for _, op in ipairs(normal_dyn_ops) do
			e("                if name == %q {", op.lbl)
			emit_dyn_blit(op, "                    ")
			e("                }")
		end
		e("            }")
		e("        }")
	end

	if #prog_ops > 0 then
		e("        for &(name, val) in changes {")
		for _, op in ipairs(prog_ops) do
			local x_prev = op.px > 0 and ("%d + prev"):format(op.px) or "prev"
			local x_filled = op.px > 0 and ("%d + filled"):format(op.px) or "filled"
			e("            if name == %q {", op.lbl)
			if op.bidir then
				local bg = rgb_lit(op.bg)
				e("                let (sym, v_raw) = if let Some(s) = val.strip_prefix('s') {")
				e("                    (true, s.parse::<f32>().unwrap_or(0.0))")
				e("                } else {")
				e("                    (false, val.parse::<f32>().unwrap_or(0.0))")
				e("                };")
				local fg
				if op.overload then
					fg = ("if (sym && v_raw.abs() >= 1.0) || (!sym && v_raw >= 1.0) { %s } else { %s }"):format(
						rgb_lit(op.overload), rgb_lit(op.fg))
				else
					fg = rgb_lit(op.fg)
				end
				e("                let fg = %s;", fg)
				e("                let bg = %s;", bg)
				e("                let v = if sym { v_raw.clamp(-1.0, 1.0) } else { v_raw.clamp(0.0, 1.0) };")
				e("                self.prog_%d_prev = 0;", op.prog_idx)
				e("                backend.fill_rect(%d, %d, %d, %d, bg);", op.px, op.py, op.pw, op.ph)
				e("                if sym {")
				e("                    let center = %d_usize;", math.floor(op.pw / 2))
				e("                    let fill = (center as f32 * v.abs()) as usize;")
				e("                    if v >= 0.0 { if fill > 0 { backend.fill_rect(%d + center, %d, fill.min(%d - center), %d, fg); } }", op.px, op.py, op.pw, op.ph)
				e("                    else { if fill > 0 { backend.fill_rect(%d + center - fill, %d, fill, %d, fg); } }", op.px, op.py, op.ph)
				e("                } else {")
				e("                    let filled = (%d.0_f32 * v) as usize;", op.pw)
				e("                    if filled > 0 { backend.fill_rect(%d, %d, filled, %d, fg); }", op.px, op.py, op.ph)
				e("                }")
			else
				e("                if let Ok(v) = val.parse::<f32>() {")
				if op.adjust then
					local min = op.adjust[2]
					local max = op.adjust[3]
					e("                    let v = ((v - %g.0) / (%g.0 - %g.0)).clamp(0.0, 1.0);", min, max, min)
				else
					e("                    let v = v.clamp(0.0, 1.0);")
				end
				local fg
				if op.overload then
					fg = ("if v >= 1.0 { %s } else { %s }"):format(rgb_lit(op.overload), rgb_lit(op.fg))
				else
					fg = rgb_lit(op.fg)
				end
				e("                    let fg = %s;", fg)
				e("                    let filled = (%d.0_f32 * v) as usize;", op.pw)
				if op.overload then
					e("                    self.prog_%d_prev = filled;", op.prog_idx)
					e("                    if filled > 0 { backend.fill_rect(%d, %d, filled, %d, fg); }", op.px, op.py, op.ph)
					e("                    if filled < %d { backend.fill_rect(%s, %d, %d - filled, %d, %s); }", op.pw, x_filled, op.py, op.pw, op.ph, rgb_lit(op.bg))
				else
					e("                    let prev = self.prog_%d_prev; self.prog_%d_prev = filled;", op.prog_idx, op.prog_idx)
					e("                    if filled != prev {")
					e("                        if prev == usize::MAX {")
					e("                            if filled > 0 { backend.fill_rect(%d, %d, filled, %d, fg); }", op.px, op.py, op.ph)
					e("                            if filled < %d { backend.fill_rect(%s, %d, %d - filled, %d, %s); }", op.pw, x_filled, op.py, op.pw, op.ph, rgb_lit(op.bg))
					e("                        } else if filled > prev {")
					e("                            backend.fill_rect(%s, %d, filled - prev, %d, fg);", x_prev, op.py, op.ph)
					e("                        } else {")
					e("                            backend.fill_rect(%s, %d, prev - filled, %d, %s);", x_filled, op.py, op.ph, rgb_lit(op.bg))
					e("                        }")
					e("                    }")
				end
				e("                }")
			end
			e("            }")
		end
		e("        }")
	end

	-- Pointer slider rendering
	local slider_ops = {}
	for _, op in ipairs(ops) do
		if op.kind == "slider" then slider_ops[#slider_ops+1] = op end
	end
	if #slider_ops > 0 then
		e("        for &(name, val) in changes {")
		for _, op in ipairs(slider_ops) do
			local tri_h = math.floor(op.sh * op.pointer_weight)
			local ribbon_h = op.sh - tri_h
			local tri_w = tri_h * 2 + 1
			local ribbon_y = op.sy + tri_h
			local bg = rgb_lit(op.bg)
			e("            if name == %q {", op.lbl)
			e("                let v = val.parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0);")
			e("                let ribbon = SLIDER_%d_RIBBON;", op.slider_idx)
			e("                let pos = (%d.0_f32 * v) as usize;", op.sw - 1)
			e("                let prev = self.slider_%d_prev; self.slider_%d_prev = pos;", op.slider_idx, op.slider_idx)
			e("                if prev == usize::MAX {")
			e("                    for xi in 0..%d_usize {", op.sw)
			e("                        let c = ribbon[xi.min(ribbon.len() - 1)];")
			e("                        backend.fill_rect(%d + xi, %d, 1, %d, c);", op.sx, ribbon_y, ribbon_h)
			e("                    }")
			e("                }")
			e("                if prev != usize::MAX && prev != pos {")
			e("                    let old_left = prev.saturating_sub(%d);", (tri_w - 1) / 2)
			e("                    let old_right = (prev + %d + 1).min(%d);", (tri_w - 1) / 2, op.sw)
			e("                    backend.fill_rect(%d + old_left, %d, old_right - old_left, %d, %s);",
				op.sx, op.sy, tri_h, bg)
			e("                }")
			do
				local fg = rgb_lit(op.fg)
				for row = 0, tri_h - 1 do
					local half = tri_h - 1 - row
					if half > 0 then
						e("                { let cx = pos as isize; let l = (cx - %d).max(0) as usize; let r = (cx + %d + 1).min(%d) as usize; if r > l { backend.fill_rect(%d + l, %d, r - l, 1, %s); } }",
							half, half, op.sw, op.sx, op.sy + row, fg)
					else
						e("                { backend.fill_rect(%d + pos, %d, 1, 1, %s); }",
							op.sx, op.sy + row, fg)
					end
				end
			end
			e("            }")
		end
		e("        }")
	end

	-- Chart widget updates
	local menu_charts = {}
	for _, op in ipairs(ops) do
		if op.kind == "chart" then menu_charts[#menu_charts+1] = op end
	end
	if #menu_charts > 0 then
		e("        for &(name, val) in changes {")
		for _, op in ipairs(menu_charts) do
			e("            if name == %q {", op.lbl)
			e("                if let Ok(v) = val.parse::<f64>() {")
			e("                    self.chart_%d_data.push(v);", op.chart_idx)
			e("                    let max_pts = %d_usize;", op.cw)
			e("                    if self.chart_%d_data.len() > max_pts { let excess = self.chart_%d_data.len() - max_pts; self.chart_%d_data.drain(..excess); }", op.chart_idx, op.chart_idx, op.chart_idx)
			e("                    self.chart_%d_dirty = true;", op.chart_idx)
			e("                }")
			e("            }")
		end
		e("        }")
		for _, op in ipairs(menu_charts) do
			local fg = rgb_lit(op.fg)
			local bg = rgb_lit(op.bg)
			e("        if self.chart_%d_dirty { self.chart_%d_dirty = false;", op.chart_idx, op.chart_idx)
			e("            let data = &self.chart_%d_data;", op.chart_idx)
			e("            let bg = %s;", bg)
			e("            let fg = %s;", fg)
			e("            backend.fill_rect(%d, %d, %d, %d, bg);", op.cx, op.cy, op.cw, op.ch)
			e("            let visible = data.len().min(%d);", op.cw)
			e("            if visible > 0 {")
			e("                let start = data.len() - visible;")
			e("                let mut min_v = f64::MAX;")
			e("                let mut max_v = f64::MIN;")
			e("                for &v in &data[start..] { min_v = min_v.min(v); max_v = max_v.max(v); }")
			e("                let range = max_v - min_v;")
			e("                if range < 1e-15 { min_v -= 1.0; max_v += 1.0; }")
			e("                else { min_v -= range * 0.1; max_v += range * 0.1; }")
			e("                let draw_h = %d_usize;", op.ch)
			e("                let scale = (draw_h.saturating_sub(1)) as f64 / (max_v - min_v);")
			if op.chart_type == "line" then
				-- Wu anti-aliased line chart (1px)
				e("                let (fr, fgc, fb) = fg.to_rgb();")
				e("                let (br, bgc, bb) = bg.to_rgb();")
				e("                let blend = |intensity: u8| -> Pixel {")
				e("                    let a = intensity as u16;")
				e("                    let ia = 255 - a;")
				e("                    Pixel::from_rgb(")
				e("                        ((fr as u16 * a + br as u16 * ia) / 255) as u8,")
				e("                        ((fgc as u16 * a + bgc as u16 * ia) / 255) as u8,")
				e("                        ((fb as u16 * a + bb as u16 * ia) / 255) as u8,")
				e("                    )")
				e("                };")
				e("                let mut prev_yf: Option<f64> = None;")
				e("                for i in 0..visible {")
				e("                    let v = data[start + i];")
				e("                    let yf = (v - min_v) * scale;")
				e("                    let px = %d + %d - visible + i;", op.cx, op.cw)
				e("                    if let Some(py) = prev_yf {")
				e("                        let dy = yf - py;")
				e("                        let steps = (dy.abs().ceil() as usize).max(1);")
				e("                        for s in 1..steps {")
				e("                            let t = s as f64 / steps as f64;")
				e("                            let ym = py + dy * t;")
				e("                            let iy = ym.floor() as usize;")
				e("                            let frac = ym - ym.floor();")
				e("                            if iy < draw_h {")
				e("                                let dpy = %d + draw_h - 1 - iy;", op.cy)
				e("                                backend.fill_rect(px - 1, dpy, 1, 1, blend(((1.0 - frac) * 255.0) as u8));")
				e("                            }")
				e("                            if iy + 1 < draw_h {")
				e("                                let dpy = %d + draw_h - 2 - iy;", op.cy)
				e("                                backend.fill_rect(px - 1, dpy, 1, 1, blend((frac * 255.0) as u8));")
				e("                            }")
				e("                        }")
				e("                    }")
				e("                    let iy = yf.floor() as usize;")
				e("                    let frac = yf - yf.floor();")
				e("                    if iy < draw_h {")
				e("                        let dpy = %d + draw_h - 1 - iy;", op.cy)
				e("                        backend.fill_rect(px, dpy, 1, 1, blend(((1.0 - frac) * 255.0) as u8));")
				e("                    }")
				e("                    if iy + 1 < draw_h {")
				e("                        let dpy = %d + draw_h - 2 - iy;", op.cy)
				e("                        backend.fill_rect(px, dpy, 1, 1, blend((frac * 255.0) as u8));")
				e("                    }")
				e("                    prev_yf = Some(yf);")
				e("                }")
			else
				-- Bar chart (default)
				e("                for i in 0..visible {")
				e("                    let v = data[start + i];")
				e("                    let norm = ((v - min_v) * scale) as usize;")
				e("                    let py = %d + draw_h - 1 - norm.min(draw_h - 1);", op.cy)
				e("                    let bar_h = (%d + draw_h - py).max(1);", op.cy)
				e("                    let px = %d + %d - visible + i;", op.cx, op.cw)
				e("                    backend.fill_rect(px, py, 1, bar_h, fg);")
				e("                }")
			end
			e("            }")
			e("        }")
		end
	end

	-- Focus redraw
	if has_focus then
		e("        if self.last_drawn_focus != self.focused {")
		e("            let prev = self.last_drawn_focus;")
		e("            let focused = self.focused;")
		e("            self.last_drawn_focus = focused;")
		e("            self.draw_focus(backend, prev, focused);")
		-- Reset dynamic label tracking for elements that overlap with changed focus
		for _, op in ipairs(dyn_ops) do
			for fi, fop in ipairs(fops) do
				local fidx = fop.focus_index
				if op.x >= fop.x and op.x < fop.x + fop.w
				   and op.text_y >= fop.y and op.text_y < fop.y + fop.h then
					e("            if prev == Some(%d) || focused == Some(%d) {", fidx, fidx)
					e("                self.dyn_%d_end = usize::MAX;", op.dyn_idx)
					e("            }")
					break
				end
			end
		end
		-- Reset progress bars that overlap with changed focus
		for _, op in ipairs(prog_ops) do
			if not op.is_focusable then
				for fi, fop in ipairs(fops) do
					local fidx = fop.focus_index
					if op.px >= fop.x and op.px < fop.x + fop.w
					   and op.py >= fop.y and op.py < fop.y + fop.h then
						e("            if prev == Some(%d) || focused == Some(%d) {", fidx, fidx)
						e("                self.prog_%d_prev = usize::MAX;", op.prog_idx)
						e("            }")
						break
					end
				end
			end
		end
		e("        }")
	end

	-- Auto-activate set_stay/set_param buttons BEFORE active state redraw
	if has_auto_active then
		e("        for &(name, val) in changes {")
		for _, param in ipairs(aa_params) do
			local group = aa[param]
			e("            if name == %q {", param)
			for _, entry in ipairs(group) do
				e("                self.active.remove(&%d);", entry.idx)
			end
			e("                match val {")
			for _, entry in ipairs(group) do
				e("                    %q => { self.active.insert(%d); }", entry.value, entry.idx)
				local info = param_info[param]
				if info and info.opts then
					local idx = tonumber(entry.value)
					if idx and info.opts[idx + 1] and info.opts[idx + 1] ~= entry.value then
						e("                    %q => { self.active.insert(%d); }", info.opts[idx + 1], entry.idx)
					end
				end
			end
			e("                    _ => {}")
			e("                }")
			e("            }")
		end
		e("        }")
	end

	-- Build lookups: container_active idx -> lists of static/dynamic children
	local container_static_children = {}
	local container_dyn_children = {}
	for _, op in ipairs(ops) do
		if op.kind == "container_active" and op.active_idx then
			container_static_children[op.active_idx] = {}
			container_dyn_children[op.active_idx] = {}
		end
	end
	for _, op in ipairs(ops) do
		if op.active_idx then
			if op.kind == "static" then
				local t = container_static_children[op.active_idx]
				if t then t[#t+1] = op end
			elseif op.kind == "dynamic" then
				local t = container_dyn_children[op.active_idx]
				if t then t[#t+1] = op end
			end
		end
	end

	-- Active state redraw for static nodes
	if #m_active_ops > 0 then
		e("        if self.active != self.last_drawn_active {")
		e("            self.last_drawn_active = self.active.clone();")
		for _, op in ipairs(m_active_ops) do
			local bg = op.atlas and rgb_lit(op.atlas.bg) or rgb_lit(op.bg)
			local act = op.active_style
			local act_bg = act.bg and rgb_lit(act.bg) or bg
			local act_border = act.border or op.normal_border
			e("            if self.active.contains(&%d) {", op.active_idx)
			e("                backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, act_bg)
			if op.lines then
				local act_atlas_fn = op.active_atlas and op.active_atlas.fn_name or op.atlas.fn_name
				for i, line in ipairs(op.lines) do
					local ly = op.text_y + (i - 1) * op.line_step
					emit_static_blit("                ", act_atlas_fn, op.align,
						op.text_x, op.inner_w, ly, line)
				end
			end
			local children = container_static_children[op.active_idx]
			if children then
				for _, ch in ipairs(children) do
					local ch_atlas = ch.active_atlas and ch.active_atlas.fn_name or ch.atlas.fn_name
					for i, line in ipairs(ch.lines) do
						local ly = ch.text_y + (i - 1) * ch.line_step
						emit_static_blit("                ", ch_atlas, ch.align,
							ch.text_x, ch.inner_w, ly, line)
					end
				end
			end
			local dyn_ch = container_dyn_children[op.active_idx]
			if dyn_ch then
				for _, dc in ipairs(dyn_ch) do
					e("                self.dyn_%d_end = usize::MAX;", dc.dyn_idx)
				end
			end
			if op.kind == "dynamic" and op.dyn_idx then
				e("                self.dyn_%d_end = usize::MAX;", op.dyn_idx)
			end
			emit_border("                ", op.x, op.y, op.w, op.h, act_border)
			e("            } else {")
			e("                backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, bg)
			if op.lines then
				for i, line in ipairs(op.lines) do
					local ly = op.text_y + (i - 1) * op.line_step
					emit_static_blit("                ", op.atlas.fn_name, op.align,
						op.text_x, op.inner_w, ly, line)
				end
			end
			if children then
				for _, ch in ipairs(children) do
					for i, line in ipairs(ch.lines) do
						local ly = ch.text_y + (i - 1) * ch.line_step
						emit_static_blit("                ", ch.atlas.fn_name, ch.align,
							ch.text_x, ch.inner_w, ly, line)
					end
				end
			end
			if dyn_ch then
				for _, dc in ipairs(dyn_ch) do
					e("                self.dyn_%d_end = usize::MAX;", dc.dyn_idx)
				end
			end
			if op.kind == "dynamic" and op.dyn_idx then
				e("                self.dyn_%d_end = usize::MAX;", op.dyn_idx)
			end
			emit_border("                ", op.x, op.y, op.w, op.h, op.normal_border)
			e("            }")
		end
		e("        }")
	end

	e("    }")  -- end update_params
	e("")
	e("} // impl State")
	switch_file("mod.rs")
	e("")
end

-------------------------------------------------------------------------------
-- 6d. Router struct
-------------------------------------------------------------------------------

e("pub struct Router {")
e("    current: Option<Menu>,")
for _, name in ipairs(menu_names) do
	e("    %s: menu_%s::State,", name:lower(), name:lower())
end
e("    press_x: usize,")
e("    press_right: usize,")
e("}")
e("")

-- Helper: emit match/if-let on self.current for a filtered subset of menus.
-- arms is a list of {name=..., ...} entries. emit_arm(arm, indent) emits the body.
-- When there is exactly one arm, emits `if let` instead of `match`.
local function emit_current_dispatch(self_ref, arms, emit_arm, indent)
	indent = indent or "    "
	if #arms == 0 then return end
	if #arms == 1 then
		local arm = arms[1]
		e("%sif let Some(Menu::%s) = %s {", indent, arm.name, self_ref)
		emit_arm(arm, indent .. "    ")
		e("%s}", indent)
	else
		e("%smatch %s {", indent, self_ref)
		for _, arm in ipairs(arms) do
			e("%s    Some(Menu::%s) => {", indent, arm.name)
			emit_arm(arm, indent .. "        ")
			e("%s    }", indent)
		end
		e("%s    _ => {}", indent)
		e("%s}", indent)
	end
end

e("impl Router {")

-- Router::new()
e("pub fn new() -> Self {")
e("    Router {")
e("        current: None,")
for _, name in ipairs(menu_names) do
	e("        %s: menu_%s::State::new(),", name:lower(), name:lower())
end
e("        press_x: 0,")
e("        press_right: 0,")
e("    }")
e("}")
e("")

-- Router::update_events()
e("pub fn update_events<C: Callbacks>(&mut self, events: &[InputEvent], state: &mut C) {")
e("    for ev in events {")
e("        if let InputEvent::Quit = ev { state.quit(); return; }")
e("    }")
e("    let mut press = (self.press_x, self.press_right);")
e("    match self.current {")
for _, name in ipairs(menu_names) do
	e("        Some(Menu::%s) => self.%s.update_events(events, state, &mut press),", name, name:lower())
end
e("        None => {}")
e("    }")
e("    self.press_x = press.0;")
e("    self.press_right = press.1;")
e("}")
e("")

-- Router::update_menu()
e("pub fn update_menu(&mut self, backend: &mut dyn Backend<Pixel>, menu: Menu) {")
e("    if self.current != Some(menu) {")
e("        self.current = Some(menu);")
e("        match menu {")
for _, name in ipairs(menu_names) do
	e("            Menu::%s => { self.%s.reset(); self.%s.draw(backend); }", name, name:lower(), name:lower())
end
e("        }")
e("    }")
e("}")
e("")

-- Router::force_redraw()
e("pub fn force_redraw(&mut self) {")
e("    self.current = None;")
e("}")
e("")

-- Router::update_params()
e("pub fn update_params(&mut self, backend: &mut dyn Backend<Pixel>, changes: &[(&str, &str)]) {")
e("    match self.current {")
for _, name in ipairs(menu_names) do
	e("        Some(Menu::%s) => self.%s.update_params(backend, changes),", name, name:lower())
end
e("        None => {}")
e("    }")
e("    backend.flush();")
e("}")
e("")

-- Router::last_press()
e("pub fn last_press(&self) -> (usize, usize) { (self.press_x, self.press_right) }")
e("")

-- Router::set_active()
do
	local active_menus = {}
	for _, name in ipairs(menu_names) do
		if #menu_active_ops[name] > 0 then
			active_menus[#active_menus+1] = { name = name }
		end
	end
	if #active_menus > 0 then
		e("pub fn set_active(&mut self, id: &str, active: bool) {")
		emit_current_dispatch("self.current", active_menus, function(arm, indent)
			e("%slet idx = match id {", indent)
			for _, op in ipairs(menu_active_ops[arm.name]) do
				e("%s    %q => Some(%d),", indent, op.active_id, op.active_idx)
			end
			e("%s    _ => None,", indent)
			e("%s};", indent)
			e("%sif let Some(i) = idx {", indent)
			e("%s    if active { self.%s.active.insert(i); } else { self.%s.active.remove(&i); }", indent, arm.name:lower(), arm.name:lower())
			e("%s}", indent)
		end)
		e("}")
		e("")
		e("pub fn clear_all_active(&mut self) {")
		emit_current_dispatch("self.current", active_menus, function(arm, indent)
			e("%sself.%s.active.clear();", indent, arm.name:lower())
		end)
		e("}")
		e("")
	end
end

-- Router::init_auto_active()
do
	local aa_menus = {}
	for _, name in ipairs(menu_names) do
		if next(auto_active[name]) then
			aa_menus[#aa_menus+1] = { name = name }
		end
	end
	if #aa_menus > 0 then
		e("pub fn init_auto_active(&mut self, values: &[(&str, &str)]) {")
		emit_current_dispatch("self.current", aa_menus, function(arm, indent)
			local aa = auto_active[arm.name]
			local aa_params = {}
			for param in pairs(aa) do aa_params[#aa_params+1] = param end
			table.sort(aa_params)
			e("%sfor &(name, val) in values {", indent)
			for _, param in ipairs(aa_params) do
				local group = aa[param]
				e("%s    if name == %q {", indent, param)
				for _, entry in ipairs(group) do
					e("%s        self.%s.active.remove(&%d);", indent, arm.name:lower(), entry.idx)
				end
				e("%s        match val {", indent)
				for _, entry in ipairs(group) do
					e("%s            %q => { self.%s.active.insert(%d); }", indent, entry.value, arm.name:lower(), entry.idx)
					local info = param_info[param]
					if info and info.opts then
						local idx = tonumber(entry.value)
						if idx and info.opts[idx + 1] and info.opts[idx + 1] ~= entry.value then
							e("%s            %q => { self.%s.active.insert(%d); }", indent, info.opts[idx + 1], arm.name:lower(), entry.idx)
						end
					end
				end
				e("%s            _ => {}", indent)
				e("%s        }", indent)
				e("%s    }", indent)
			end
			e("%s}", indent)
		end)
		e("}")
		e("")
	end
end

-- Router::set_focused() and friends — only if any menu has focusable ops with labels
do
	local entries = {}
	for _, name in ipairs(menu_names) do
		local fops = get_focusable_ops(menu_ops[name])
		for fi, op in ipairs(fops) do
			if op.lbl then
				entries[#entries+1] = { menu = name, idx = op.focus_index, lbl = op.lbl }
			end
		end
	end
	if #entries > 0 then
		e("pub fn set_focused(&mut self, label: &str) {")
		-- Group by menu
		local by_menu = {}
		local focus_menus = {}
		for _, ent in ipairs(entries) do
			if not by_menu[ent.menu] then
				by_menu[ent.menu] = {}
				focus_menus[#focus_menus+1] = { name = ent.menu }
			end
			by_menu[ent.menu][#by_menu[ent.menu]+1] = ent
		end
		emit_current_dispatch("self.current", focus_menus, function(arm, indent)
			e("%sself.%s.focused = match label {", indent, arm.name:lower())
			for _, ent in ipairs(by_menu[arm.name]) do
				e("%s    %q => Some(%d),", indent, ent.lbl, ent.idx)
			end
			e("%s    _ => None,", indent)
			e("%s};", indent)
		end)
		e("}")
		e("")
		e("pub fn clear_focused(&mut self) {")
		e("    match self.current {")
		for _, name in ipairs(menu_names) do
			e("        Some(Menu::%s) => self.%s.focused = None,", name, name:lower())
		end
		e("        _ => {}")
		e("    }")
		e("}")
		e("")
		e("pub fn get_focused(&self) -> Option<usize> {")
		e("    match self.current {")
		for _, name in ipairs(menu_names) do
			e("        Some(Menu::%s) => self.%s.focused,", name, name:lower())
		end
		e("        _ => None,")
		e("    }")
		e("}")
		e("")
		e("pub fn set_focused_raw(&mut self, idx: Option<usize>) {")
		e("    match self.current {")
		for _, name in ipairs(menu_names) do
			e("        Some(Menu::%s) => self.%s.focused = idx,", name, name:lower())
		end
		e("        _ => {}")
		e("    }")
		e("}")
		e("")
	end
end

-- Router::focused_adjust()
do
	local adjustables = {}
	for _, name in ipairs(menu_names) do
		local fops = get_focusable_ops(menu_ops[name])
		for fi, op in ipairs(fops) do
			if op.adjust then
				adjustables[#adjustables+1] = {
					menu = name,
					focus_idx = op.focus_index,
					param = op.adjust[1],
					min = op.adjust[2],
					max = op.adjust[3],
				}
			end
		end
	end
	if #adjustables > 0 then
		e("pub fn focused_adjust(&self) -> Option<(&'static str, f64, f64)> {")
		e("    match self.current {")
		-- Group by menu
		local by_menu = {}
		for _, a in ipairs(adjustables) do
			if not by_menu[a.menu] then by_menu[a.menu] = {} end
			by_menu[a.menu][#by_menu[a.menu]+1] = a
		end
		for _, name in ipairs(menu_names) do
			if by_menu[name] then
				e("        Some(Menu::%s) => match self.%s.focused {", name, name:lower())
				for _, a in ipairs(by_menu[name]) do
					e("            Some(%d) => Some((%q, %g.0, %g.0)),", a.focus_idx, a.param, a.min, a.max)
				end
				e("            _ => None,")
				e("        },")
			end
		end
		e("        _ => None,")
		e("    }")
		e("}")
		e("")
	end
end

-- Router::label_bounds()
do
	local entries = {}
	for _, name in ipairs(menu_names) do
		for _, op in ipairs(menu_ops[name]) do
			if op.kind == "dynamic" and op.lbl then
				entries[#entries+1] = { menu = name, lbl = op.lbl, x = op.x, y = op.y, w = op.w, h = op.h }
			end
		end
	end
	if #entries > 0 then
		e("pub fn label_bounds(&self, label: &str) -> Option<(usize, usize, usize, usize)> {")
		e("    match self.current {")
		local by_menu = {}
		for _, ent in ipairs(entries) do
			if not by_menu[ent.menu] then by_menu[ent.menu] = {} end
			by_menu[ent.menu][#by_menu[ent.menu]+1] = ent
		end
		for _, name in ipairs(menu_names) do
			if by_menu[name] then
				e("        Some(Menu::%s) => match label {", name)
				for _, ent in ipairs(by_menu[name]) do
					e("            %q => Some((%d, %d, %d, %d)),", ent.lbl, ent.x, ent.y, ent.w, ent.h)
				end
				e("            _ => None,")
				e("        },")
			end
		end
		e("        _ => None,")
		e("    }")
		e("}")
		e("")
	end
end

e("} // impl Router")
e("")

-------------------------------------------------------------------------------
-- 6e. Free functions (stateless)
-------------------------------------------------------------------------------

-- emit FMT_PARAMS
do
	local fmt_params = {}
	for _, name in ipairs(menu_names) do
		for _, op in ipairs(menu_ops[name]) do
			if op.kind == "dynamic" and op.fmt and op.lbl then
				fmt_params[op.lbl] = true
			end
		end
	end
	for name, _ in pairs(param_info) do
		fmt_params[name] = true
	end
	local sorted = {}
	for k in pairs(fmt_params) do sorted[#sorted+1] = k end
	table.sort(sorted)
	if #sorted > 0 then
		e("pub const FMT_PARAMS: &[&str] = &[%s];",
			table.concat(
				(function() local t = {} for _, v in ipairs(sorted) do t[#t+1] = ("%q"):format(v) end return t end)(),
				", "))
		e("")
	end
end

-- emit format_param()
do
	local used_params = {}
	for _, name in ipairs(menu_names) do
		for _, op in ipairs(menu_ops[name]) do
			if op.kind == "dynamic" and op.lbl and param_info[op.lbl] and not op.fmt then
				used_params[op.lbl] = true
			end
			if op.derived then
				for _, src in ipairs(op.derived.sources) do
					if param_info[src] then
						used_params[src] = true
					end
				end
			end
		end
	end
	local sorted = {}
	for k in pairs(used_params) do sorted[#sorted+1] = k end
	table.sort(sorted)

	local MINUS = "\u{2212}"

	if #sorted > 0 then
		e("/// Format a parameter value using metadata from params.txt.")
		e("/// Returns None if the parameter has no built-in formatting.")
		e("pub fn format_param(name: &str, val: &str) -> Option<String> {")
		e("    match name {")
		for _, pname in ipairs(sorted) do
		local info = param_info[pname]
		if info.opts then
			e("        %q => {", pname)
			e("            let i = val.parse::<f64>().ok()? as usize;")
			e("            Some(match i {")
			for i, opt in ipairs(info.opts) do
				e("                %d => %q.to_owned(),", i - 1, opt)
			end
			e("                _ => val.to_owned(),")
			e("            })")
			e("        }")
		elseif info.prec then
			local prec = info.prec
			local unit = info.unit
			local is_angle = info.is_angle
			e("        %q => {", pname)
			e("            let n = val.parse::<f64>().ok()?;")
			if is_angle then
				e("            let raw = if n < 0.0 {")
				e("                format!(\"-{:0>width$.prec$}\", n.abs(), width = %d, prec = %d)", prec + 4, prec)
				e("            } else {")
				e("                format!(\"{:0>width$.prec$}\", n, width = %d, prec = %d)", prec + 4, prec)
				e("            };")
			else
				e("            let raw = format!(\"{:.%d}\", n);", prec)
			end
			e("            let formatted = if let Some(dot) = raw.find('.') {")
			e("                let (int_str, dec_with_dot) = raw.split_at(dot);")
			e("                let dec_part = &dec_with_dot[1..];")
			e("                let grouped_dec: String = dec_part.chars().enumerate().map(|(i, c)| {")
			e("                    if i > 0 && i % 3 == 0 { format!(\" {}\", c) } else { c.to_string() }")
			e("                }).collect();")
			e("                let grouped_int = crate::group_int_digits(int_str);")
			e("                format!(\"{}.{}\", grouped_int, grouped_dec)")
			e("            } else {")
			e("                crate::group_int_digits(&raw)")
			e("            };")
			e("            let formatted = formatted.replacen('-', \"\\u{2212}\", 1);")
			if unit then
				if is_angle then
					e("            Some(format!(\"{}%s\", formatted))", unit)
				else
					e("            Some(format!(\"{} %s\", formatted))", unit)
				end
			else
				e("            Some(formatted)")
			end
			e("        }")
			end
		end
		e("        _ => None,")
		e("    }")
		e("}")
		e("")
	end
end

os.execute("mkdir -p " .. out_dir)
for filename, lines in pairs(files) do
	local f = io.open(out_dir .. "/" .. filename, "w")
	if not f then
		io.stderr:write("ERROR: cannot write " .. out_dir .. "/" .. filename .. "\n")
		os.exit(1)
	end
	f:write(table.concat(lines, "\n"))
	f:write("\n")
	f:close()
end
if warn_count > 0 then
	os.exit(1)
end
