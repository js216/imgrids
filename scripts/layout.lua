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
	return ("rgb!(%d, %d, %d)"):format(c[1], c[2], c[3])
end

local function is_raster(font)
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

local function char_width_est(font)
	if is_raster(font) then
		local _, glyph_w, _ = resolve_raster_font(font)
		return glyph_w
	else
		-- TTF: ~0.55 * cell_h (good approximation for RobotoMono; conservative for proportional)
		return math.max(1, math.floor(font[2] * 0.55))
	end
end

local function cell_height_est(font)
	if is_raster(font) then
		local _, _, glyph_h = resolve_raster_font(font)
		return glyph_h
	else
		return font[2]
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
	border=1, align=1,
}
local BORDER_KEYS = { width=1, color=1, side=1 }
local NODE_KEYS = {
	-- layout
	size=1, weight=1,
	-- behavior
	press=1, focusable=1, lbl=1, render=1, align=1,
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
	if s.border.side == nil or s.border.side == side then
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
	if not is_raster(font) and not checked_fonts[font[1]] then
		checked_fonts[font[1]] = true
		local f = io.open(font[1], "r")
		if f then
			f:close()
		else
			warn("font file not found: %s", font[1])
		end
	end
	local idx = #atlases + 1
	local rec = {
		key = k,
		font = font,
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
	else
		-- Leaf node
		local lbl, render, press
		local text

		if type(node) == "string" then
			text = node
		elseif type(node) == "table" then
			lbl = node.lbl
			render = node.render
			press = get_press(node)
			if not lbl and type(node[1]) == "string" and node[1] ~= "row" and node[1] ~= "col" then
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
		if type(node) == "table" and node.focusable ~= nil then
			is_focusable = node.focusable
		else
			is_focusable = (press ~= nil)
		end

		local atlas = render ~= "progress bar" and get_atlas(s.font, s.fg, s.bg) or nil
		local ch_px = cell_height_est(s.font)
		local cw_px = char_width_est(s.font)
		-- Inset by border + padding so text is drawn inside both
		local bx = border_inset(s, "left") + eff_pad(s, "left")
		local by = border_inset(s, "top") + eff_pad(s, "top")
		local bw = bx + border_inset(s, "right") + eff_pad(s, "right")
		local bh = by + border_inset(s, "bottom") + eff_pad(s, "bottom")
		local text_x = x + bx
		local inner_w = math.max(0, w - bw)
		local pad_chars = math.max(0, math.floor(inner_w / cw_px))
		-- Split text on \n; center the whole block vertically
		local line_gap = 2
		local lines = {}
		if text then
			for line in (text .. "\n"):gmatch("([^\n]*)\n") do
				lines[#lines + 1] = line
			end
		end
		local n_lines = math.max(1, #lines)
		local block_h = n_lines * ch_px + (n_lines - 1) * line_gap
		local text_y = (y + by) + math.floor(((h - bh) - block_h) / 2)
		local line_step = ch_px + line_gap
		-- Per-line x positions for static text alignment
		local line_xs = {}
		for i, line in ipairs(lines) do
			if align == "center" then
				local tw = #line * cw_px
				line_xs[i] = text_x + math.floor((inner_w - tw) / 2)
			elseif align == "right" then
				local tw = #line * cw_px
				line_xs[i] = text_x + (inner_w - tw)
			else
				line_xs[i] = text_x
			end
		end

		-- Compute focused style data for focusable cells
		local foc = nil
		if is_focusable then
			local fs = merge_style(s, focused_ovr)
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
					local tw = #line * fcw_px
					f_line_xs[i] = f_text_x + math.floor((f_inner_w - tw) / 2)
				elseif align == "right" then
					local tw = #line * fcw_px
					f_line_xs[i] = f_text_x + (f_inner_w - tw)
				else
					f_line_xs[i] = f_text_x
				end
			end
			foc = {
				atlas = get_atlas(fs.font, fs.fg, fs.bg),
				text_x = f_text_x,
				line_xs = f_line_xs,
				text_y = (y + fby) + math.floor(((h - fbh) - fblock_h) / 2),
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

		if lbl then
			if render == "progress bar" then
				local inset_l = eff_pad(s, "left") + border_inset(s, "left")
				local inset_t = eff_pad(s, "top") + border_inset(s, "top")
				local inset_r = eff_pad(s, "right") + border_inset(s, "right")
				local inset_b = eff_pad(s, "bottom") + border_inset(s, "bottom")
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
					atlas = atlas,
					pad_chars = pad_chars,
					align = align,
					inner_w = inner_w,
					press = press,
					is_focusable = is_focusable,
					foc = foc,
					normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
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
				line_xs = line_xs,
				lines = lines,
				line_step = line_step,
				atlas = atlas,
				press = press,
				is_focusable = is_focusable,
				foc = foc,
				normal_border = { width = s.border.width, color = s.border.color, side = s.border.side },
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
local out = {}
local function e(fmt, ...)
	if select("#", ...) > 0 then
		out[#out + 1] = fmt:format(...)
	else
		out[#out + 1] = fmt
	end
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

e("// Generated by scripts/layout.lua — do not edit.")
e("// Re-run the transpiler to update:")
e("//   lua scripts/layout.lua < examples/ui.lua > examples/app/ui.rs")
e("// transpiler: %s", transpiler_hash)
e("// input:      %s", input_hash)
e("")

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
		if op.kind == "dynamic" or op.kind == "static" then
			need_renderer = true
			break
		end
	end
end

e("use imgrids::{rgb, Backend, InputEvent%s};", need_renderer and ", Renderer" or "")
if need_raster then
	e("use imgrids::raster::RasterAtlas;")
end
if need_ttf then
	e("use imgrids::ttf::TtfAtlas;")
end
if #atlases > 0 then
	e("use std::sync::OnceLock;")
end
e("use std::sync::Mutex;")
e("")

-- Atlas statics and getters
for _, a in ipairs(atlases) do
	if is_raster(a.font) then
		local name, gw, gh = resolve_raster_font(a.font)
		e("static %s: OnceLock<RasterAtlas> = OnceLock::new();", a.varname)
		e("fn %s() -> &'static RasterAtlas {", a.fn_name)
		e("    %s.get_or_init(|| RasterAtlas::new(", a.varname)
		e("        &imgrids::fonts::%s::FONT, %d, %d, %s, %s,", name, gw, gh, rgb_lit(a.fg), rgb_lit(a.bg))
		e("    ))")
		e("}")
	else
		e("static %s: OnceLock<TtfAtlas> = OnceLock::new();", a.varname)
		e("fn %s() -> &'static TtfAtlas {", a.fn_name)
		e(
			"    %s.get_or_init(|| TtfAtlas::new(%q, %d, %s, %s)",
			a.varname,
			a.font[1],
			a.font[2],
			rgb_lit(a.fg),
			rgb_lit(a.bg)
		)
		e("        .expect(%q))", a.font[1])
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

-- CURRENT_MENU, FOCUSED, and LAST_DRAWN_FOCUS statics
e("static CURRENT_MENU: Mutex<Option<Menu>> = Mutex::new(None);")
e("static FOCUSED: Mutex<Option<usize>> = Mutex::new(None);")
e("static LAST_DRAWN_FOCUS: Mutex<Option<usize>> = Mutex::new(Some(usize::MAX));")
e("")

-- Collect all progress bar ops globally and assign indices
local all_prog_ops = {}
for _, name in ipairs(menu_names) do
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "progress" then
			all_prog_ops[#all_prog_ops+1] = op
			op.prog_idx = #all_prog_ops
		end
	end
end
if #all_prog_ops > 0 then
	e("use std::sync::atomic::{AtomicUsize, Ordering};")
	for i = 1, #all_prog_ops do
		e("static PROG_%d_PREV: AtomicUsize = AtomicUsize::new(usize::MAX);", i)
	end
	e("")
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

-- update_events<C: Callbacks>()
e("pub fn update_events<C: Callbacks>(events: &[InputEvent], state: &mut C) {")
e("    for ev in events {")
e("        if let InputEvent::Quit = ev { state.quit(); return; }")
e("    }")
e("    match *CURRENT_MENU.lock().unwrap() {")
for _, name in ipairs(menu_names) do
	e("        Some(Menu::%s) => update_events_%s(events, state),", name, name:lower())
end
e("        None => {}")
e("    }")
e("}")
e("")

-- update_menu()
e("pub fn update_menu(backend: &mut dyn Backend, menu: Menu) {")
e("    let mut current = CURRENT_MENU.lock().unwrap();")
e("    if *current != Some(menu) {")
e("        *current = Some(menu);")
e("        drop(current);")
e("        *FOCUSED.lock().unwrap() = None;")
e("        *LAST_DRAWN_FOCUS.lock().unwrap() = None;")
for i = 1, #all_prog_ops do
	e("        PROG_%d_PREV.store(usize::MAX, Ordering::Relaxed);", i)
end
e("        match menu {")
for _, name in ipairs(menu_names) do
	e("            Menu::%s => draw_%s(backend),", name, name:lower())
end
e("        }")
e("        backend.flush();")
e("    }")
e("}")
e("")

e("pub fn force_redraw() {")
e("    *CURRENT_MENU.lock().unwrap() = None;")
e("}")
e("")

-- update_changes()
e("pub fn update_changes(backend: &mut dyn Backend, changes: &[(&str, &str)]) {")
e("    match *CURRENT_MENU.lock().unwrap() {")
for _, name in ipairs(menu_names) do
	e("        Some(Menu::%s) => update_changes_%s(backend, changes),", name, name:lower())
end
e("        None => {}")
e("    }")
e("    backend.flush();")
e("}")
e("")

-- draw_<menu>() per-menu functions
for _, name in ipairs(menu_names) do
	e("fn draw_%s(backend: &mut dyn Backend) {", name:lower())
	e("    backend.fill_rect(0, 0, %d, %d, %s);", screen.width, screen.height, rgb_lit(default_style.bg))
	for _, op in ipairs(menu_ops[name]) do
		if op.kind == "static" then
			e("    backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.atlas.bg))
			for i, line in ipairs(op.lines) do
				e("    %s().draw(backend, %d, %d, %q);", op.atlas.fn_name, op.line_xs[i], op.text_y + (i - 1) * op.line_step, line)
			end
		elseif op.kind == "dynamic" or op.kind == "progress" then
			local bg = op.atlas and op.atlas.bg or op.bg
			e("    backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(bg))
		elseif op.kind == "fill" then
			e("    backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.color))
		elseif op.kind == "border" then
			if op.side then
				local x, y, w, h, t, c = op.x, op.y, op.w, op.h, op.thickness, rgb_lit(op.color)
				if op.side == "top" then
					e("    backend.fill_rect(%d, %d, %d, %d, %s);", x, y, w, t, c)
				elseif op.side == "bottom" then
					e("    backend.fill_rect(%d, %d, %d, %d, %s);", x, y + h - t, w, t, c)
				elseif op.side == "left" then
					e("    backend.fill_rect(%d, %d, %d, %d, %s);", x, y, t, h, c)
				elseif op.side == "right" then
					e("    backend.fill_rect(%d, %d, %d, %d, %s);", x + w - t, y, t, h, c)
				end
			else
				e(
					"    backend.draw_border(%d, %d, %d, %d, %d, %s);",
					op.x,
					op.y,
					op.w,
					op.h,
					op.thickness,
					rgb_lit(op.color)
				)
			end
		end
	end
	e("}")
	e("")
end

-- Helper: collect focusable (static/dynamic, is_focusable=true) ops in order
local function get_focusable_ops(ops)
	local fops = {}
	for _, op in ipairs(ops) do
		if (op.kind == "static" or op.kind == "dynamic") and op.is_focusable then
			fops[#fops + 1] = op
		end
	end
	return fops
end

-- Helper: emit blit code for a single dynamic label (inside render closure)
local function emit_dyn_blit(op, indent)
	if op.align == "left" then
		e("%s%s().blit(fb, stride, %d, %d,", indent, op.atlas.fn_name, op.text_x, op.text_y)
		e('%s    &format!("{:<%d}", val));', indent, op.pad_chars)
	else
		e("%slet a = %s();", indent, op.atlas.fn_name)
		-- Clear old text with spaces
		e('%sa.blit(fb, stride, %d, %d, &format!("{:<%d}", ""));', indent, op.text_x, op.text_y, op.pad_chars)
		e("%slet tw = a.text_width(val);", indent)
		if op.align == "center" then
			e("%sa.blit(fb, stride, %d + (%d_usize.saturating_sub(tw)) / 2, %d, val);",
				indent, op.text_x, op.inner_w, op.text_y)
		else -- right
			e("%sa.blit(fb, stride, %d + %d_usize.saturating_sub(tw), %d, val);",
				indent, op.text_x, op.inner_w, op.text_y)
		end
	end
end

-- Helper: emit border drawing lines (fill_rect or draw_border)
local function emit_border(indent, op_x, op_y, op_w, op_h, b)
	if b.width == 0 then
		return
	end
	local i = indent
	if b.side then
		local x2, y2, w2, h2, t, c = op_x, op_y, op_w, op_h, b.width, rgb_lit(b.color)
		if b.side == "top" then
			e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2, w2, t, c)
		elseif b.side == "bottom" then
			e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2 + h2 - t, w2, t, c)
		elseif b.side == "left" then
			e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2, y2, t, h2, c)
		elseif b.side == "right" then
			e("%sbackend.fill_rect(%d, %d, %d, %d, %s);", i, x2 + w2 - t, y2, t, h2, c)
		end
	else
		e("%sbackend.draw_border(%d, %d, %d, %d, %d, %s);", i, op_x, op_y, op_w, op_h, b.width, rgb_lit(b.color))
	end
end

-- draw_focus_<menu>() per-menu focus redraw functions
for _, name in ipairs(menu_names) do
	local fops = get_focusable_ops(menu_ops[name])
	if #fops > 0 then
		e("fn draw_focus_%s(backend: &mut dyn Backend, focused: Option<usize>) {", name:lower())
		for fi, op in ipairs(fops) do
			local idx = fi - 1
			e("    if focused == Some(%d) {", idx)
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.foc.bg))
			if op.kind == "static" then
				for i, line in ipairs(op.lines) do
					local fx = op.foc.line_xs[i] or op.foc.text_x
					e("        %s().draw(backend, %d, %d, %q);",
						op.foc.atlas.fn_name, fx,
						op.foc.text_y + (i - 1) * op.foc.line_step, line)
				end
			end
			emit_border("        ", op.x, op.y, op.w, op.h, op.foc.border)
			e("    } else {")
			e("        backend.fill_rect(%d, %d, %d, %d, %s);", op.x, op.y, op.w, op.h, rgb_lit(op.atlas.bg))
			if op.kind == "static" then
				for i, line in ipairs(op.lines) do
					local lx = op.line_xs[i] or op.text_x
					e("        %s().draw(backend, %d, %d, %q);", op.atlas.fn_name, lx, op.text_y + (i - 1) * op.line_step, line)
				end
			end
			emit_border("        ", op.x, op.y, op.w, op.h, op.normal_border)
			e("    }")
		end
		e("}")
		e("")
	end
end

-- update_events_<menu><C: Callbacks>() per-menu functions
for _, name in ipairs(menu_names) do
	local ops = menu_ops[name]
	local press_ops = {}
	for _, op in ipairs(ops) do
		if op.press then
			press_ops[#press_ops + 1] = op
		end
	end
	local fops = get_focusable_ops(ops)
	local has_press = #press_ops > 0
	local has_focus = #fops > 0

	local ev_param = (has_press or has_focus) and "events" or "_events"
	local st_param = has_press and "state" or "_state"
	e("fn update_events_%s<C: Callbacks>(%s: &[InputEvent], %s: &mut C) {", name:lower(), ev_param, st_param)

	if has_press or has_focus then
		e("    for ev in events {")
		e("        if let InputEvent::Press { x, y } = ev {")

		if has_focus then
			if #fops == 1 then
				local op = fops[1]
				local x_lo = op.x > 0 and ("*x >= %d && "):format(op.x) or ""
				local y_lo = op.y > 0 and ("*y >= %d && "):format(op.y) or ""
				e(
					"            *FOCUSED.lock().unwrap() = if %s*x < %d && %s*y < %d { Some(0) } else { None };",
					x_lo,
					op.x + op.w,
					y_lo,
					op.y + op.h
				)
			else
				e("            let new_focus =")
				for fi, op in ipairs(fops) do
					local idx = fi - 1
					local x_lo = op.x > 0 and ("*x >= %d && "):format(op.x) or ""
					local y_lo = op.y > 0 and ("*y >= %d && "):format(op.y) or ""
					local prefix = fi == 1 and "                if " or "                else if "
					e("%s%s*x < %d && %s*y < %d { Some(%d) }", prefix, x_lo, op.x + op.w, y_lo, op.y + op.h, idx)
				end
				e("                else { None };")
				e("            *FOCUSED.lock().unwrap() = new_focus;")
			end
		end

		if has_press then
			for _, op in ipairs(press_ops) do
				local fn_name = op.press[1]
				local x_lo = op.x > 0 and ("*x >= %d && "):format(op.x) or ""
				local y_lo = op.y > 0 and ("*y >= %d && "):format(op.y) or ""
				e("            if %s*x < %d && %s*y < %d {", x_lo, op.x + op.w, y_lo, op.y + op.h)
				if callbacks[fn_name] > 0 then
					local args = {}
					for i = 2, #op.press do
						args[#args + 1] = ("%q"):format(op.press[i])
					end
					e("                state.%s(&[%s]);", fn_name, table.concat(args, ", "))
				else
					e("                state.%s();", fn_name)
				end
				e("            }")
			end
		end

		e("        }")
		e("    }")
	end

	e("}")
	e("")
end

-- update_changes_<menu>() per-menu functions
for _, name in ipairs(menu_names) do
	local ops = menu_ops[name]
	local dyn_ops = {}
	local prog_ops = {}
	for _, op in ipairs(ops) do
		if op.kind == "dynamic" then
			dyn_ops[#dyn_ops + 1] = op
		end
		if op.kind == "progress" then
			prog_ops[#prog_ops + 1] = op
		end
	end
	local fops = get_focusable_ops(ops)

	local be_param = (#dyn_ops + #prog_ops + #fops) > 0 and "backend" or "_backend"
	local chg_param = (#dyn_ops + #prog_ops) > 0 and "changes" or "_changes"
	e("fn update_changes_%s(%s: &mut dyn Backend, %s: &[(&str, &str)]) {", name:lower(), be_param, chg_param)

	if #dyn_ops > 0 then
		-- Guard render() behind a check for matching label names
		local conds = {}
		for _, op in ipairs(dyn_ops) do
			conds[#conds+1] = ("n == %q"):format(op.lbl)
		end
		e("    if changes.iter().any(|&(n, _)| %s) {", table.concat(conds, " || "))
		e("        backend.render(&mut |fb, stride| {")
		e("            for &(name, val) in changes {")
		if #dyn_ops == 1 then
			local op = dyn_ops[1]
			e("                if name == %q {", op.lbl)
			emit_dyn_blit(op, "                    ")
			e("                }")
		else
			e("                match name {")
			for _, op in ipairs(dyn_ops) do
				e("                    %q => {", op.lbl)
				emit_dyn_blit(op, "                        ")
				e("                    }")
			end
			e("                    _ => {}")
			e("                }")
		end
		e("            }")
		e("        });")
		e("    }")
	end

	if #prog_ops > 0 then
		e("    for &(name, val) in changes {")
		for _, op in ipairs(prog_ops) do
			local x_prev = op.px > 0 and ("%d + prev"):format(op.px) or "prev"
			local x_filled = op.px > 0 and ("%d + filled"):format(op.px) or "filled"
			e("        if name == %q {", op.lbl)
			e("            if let Ok(v) = val.parse::<f32>() {")
			e("                let v = v.clamp(0.0, 1.0);")
			e("                let filled = (%d.0_f32 * v) as usize;", op.pw)
			e("                let prev = PROG_%d_PREV.swap(filled, Ordering::Relaxed);", op.prog_idx)
			e("                if filled != prev {")
			e("                    if prev == usize::MAX {")
			e("                        if filled > 0 { backend.fill_rect(%d, %d, filled, %d, %s); }", op.px, op.py, op.ph, rgb_lit(op.fg))
			e("                        if filled < %d { backend.fill_rect(%s, %d, %d - filled, %d, %s); }", op.pw, x_filled, op.py, op.pw, op.ph, rgb_lit(op.bg))
			e("                    } else if filled > prev {")
			e("                        backend.fill_rect(%s, %d, filled - prev, %d, %s);", x_prev, op.py, op.ph, rgb_lit(op.fg))
			e("                    } else {")
			e("                        backend.fill_rect(%s, %d, prev - filled, %d, %s);", x_filled, op.py, op.ph, rgb_lit(op.bg))
			e("                    }")
			e("                }")
			e("            }")
			e("        }")
		end
		e("    }")
	end

	if #fops > 0 then
		e("    {")
		e("        let focused = *FOCUSED.lock().unwrap();")
		e("        let mut last = LAST_DRAWN_FOCUS.lock().unwrap();")
		e("        if *last != focused {")
		e("            *last = focused;")
		e("            drop(last);")
		e("            draw_focus_%s(backend, focused);", name:lower())
		e("        }")
		e("    }")
	end

	e("}")
	e("")
end

io.write(table.concat(out, "\n"))
io.write("\n")
if warn_count > 0 then
	os.exit(1)
end
