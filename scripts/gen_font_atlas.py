#!/usr/bin/env python3
"""Generate pre-baked font atlas alpha data as a Rust source file.

Input:  JSON on stdin — list of atlas specs:
  [{"id": "atlas_1", "fonts": [["path.ttf", 28], ...], "extra": [0xB1, ...]}]

Output: Rust source on stdout with const arrays per atlas:
  ATLAS_1_CELL_H, ATLAS_1_ASCII_ADV, ATLAS_1_ASCII_OFF, ATLAS_1_EXT, ATLAS_1_ALPHA

Uses FreeType via ctypes (libfreetype.so.6 must be installed).
No external Python packages required.
"""

import ctypes
import ctypes.util
import json
import sys
import struct

# --- FreeType bindings via ctypes ---

_ft_lib = None

def _load_freetype():
    global _ft_lib
    if _ft_lib is not None:
        return _ft_lib
    name = ctypes.util.find_library("freetype")
    if name is None:
        # Try common names directly
        for n in ("libfreetype.so.6", "libfreetype.so", "libfreetype.dylib"):
            try:
                _ft_lib = ctypes.CDLL(n)
                return _ft_lib
            except OSError:
                continue
        raise RuntimeError("Cannot find libfreetype")
    _ft_lib = ctypes.CDLL(name)
    return _ft_lib

class FT_Vector(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]

class FT_Bitmap(ctypes.Structure):
    _fields_ = [
        ("rows", ctypes.c_int),
        ("width", ctypes.c_int),
        ("pitch", ctypes.c_int),
        ("buffer", ctypes.POINTER(ctypes.c_ubyte)),
        ("num_grays", ctypes.c_short),
        ("pixel_mode", ctypes.c_ubyte),
        ("palette_mode", ctypes.c_ubyte),
        ("palette", ctypes.c_void_p),
    ]

class FT_GlyphSlotRec(ctypes.Structure):
    pass

class FT_Glyph_Metrics(ctypes.Structure):
    _fields_ = [
        ("width", ctypes.c_long), ("height", ctypes.c_long),
        ("horiBearingX", ctypes.c_long), ("horiBearingY", ctypes.c_long),
        ("horiAdvance", ctypes.c_long),
        ("vertBearingX", ctypes.c_long), ("vertBearingY", ctypes.c_long),
        ("vertAdvance", ctypes.c_long),
    ]

FT_GlyphSlotRec._fields_ = [
    ("library", ctypes.c_void_p),
    ("face", ctypes.c_void_p),
    ("next", ctypes.c_void_p),
    ("glyph_index", ctypes.c_uint),
    ("generic_data", ctypes.c_void_p),
    ("generic_finalizer", ctypes.c_void_p),
    ("metrics", FT_Glyph_Metrics),
    ("linearHoriAdvance", ctypes.c_long),
    ("linearVertAdvance", ctypes.c_long),
    ("advance", FT_Vector),
    ("format", ctypes.c_uint),
    ("bitmap", FT_Bitmap),
    ("bitmap_left", ctypes.c_int),
    ("bitmap_top", ctypes.c_int),
]

class FT_Size_Metrics(ctypes.Structure):
    _fields_ = [
        ("x_ppem", ctypes.c_ushort), ("y_ppem", ctypes.c_ushort),
        ("x_scale", ctypes.c_long), ("y_scale", ctypes.c_long),
        ("ascender", ctypes.c_long), ("descender", ctypes.c_long),
        ("height", ctypes.c_long), ("max_advance", ctypes.c_long),
    ]

class FT_SizeRec(ctypes.Structure):
    _fields_ = [
        ("face", ctypes.c_void_p),
        ("generic_data", ctypes.c_void_p),
        ("generic_finalizer", ctypes.c_void_p),
        ("metrics", FT_Size_Metrics),
    ]

class FT_FaceRec(ctypes.Structure):
    _fields_ = [
        ("num_faces", ctypes.c_long), ("face_index", ctypes.c_long),
        ("face_flags", ctypes.c_long), ("style_flags", ctypes.c_long),
        ("num_glyphs", ctypes.c_long),
        ("family_name", ctypes.c_char_p), ("style_name", ctypes.c_char_p),
        ("num_fixed_sizes", ctypes.c_int), ("available_sizes", ctypes.c_void_p),
        ("num_charmaps", ctypes.c_int), ("charmaps", ctypes.c_void_p),
        ("generic_data", ctypes.c_void_p), ("generic_finalizer", ctypes.c_void_p),
        ("bbox_xMin", ctypes.c_long), ("bbox_yMin", ctypes.c_long),
        ("bbox_xMax", ctypes.c_long), ("bbox_yMax", ctypes.c_long),
        ("units_per_EM", ctypes.c_ushort),
        ("ascender", ctypes.c_short), ("descender", ctypes.c_short),
        ("height", ctypes.c_short),
        ("max_advance_width", ctypes.c_short), ("max_advance_height", ctypes.c_short),
        ("underline_position", ctypes.c_short), ("underline_thickness", ctypes.c_short),
        ("glyph", ctypes.POINTER(FT_GlyphSlotRec)),
        ("size", ctypes.POINTER(FT_SizeRec)),
        ("charmap", ctypes.c_void_p),
    ]

FT_LOAD_RENDER = 4
FT_LOAD_NO_HINTING = 2


class FreeType:
    def __init__(self):
        self.lib = _load_freetype()
        self._library = ctypes.c_void_p()
        rc = self.lib.FT_Init_FreeType(ctypes.byref(self._library))
        if rc != 0:
            raise RuntimeError(f"FT_Init_FreeType failed: {rc}")

    def load_face(self, path, index=0):
        face = ctypes.POINTER(FT_FaceRec)()
        rc = self.lib.FT_New_Face(self._library, path.encode(), index, ctypes.byref(face))
        if rc != 0:
            raise RuntimeError(f"FT_New_Face({path}) failed: {rc}")
        return face

    def set_pixel_sizes(self, face, w, h):
        rc = self.lib.FT_Set_Pixel_Sizes(face, w, h)
        if rc != 0:
            raise RuntimeError(f"FT_Set_Pixel_Sizes failed: {rc}")

    def get_char_index(self, face, charcode):
        return self.lib.FT_Get_Char_Index(face, charcode)

    def load_glyph(self, face, glyph_index, flags=FT_LOAD_RENDER):
        rc = self.lib.FT_Load_Glyph(face, glyph_index, flags)
        if rc != 0:
            raise RuntimeError(f"FT_Load_Glyph failed: {rc}")


# --- Atlas generation ---

# Fallback map: Unicode → ASCII equivalent for missing glyphs
FALLBACKS = {0x2212: 0x002D, 0x00B7: 0x002E}
# Standard ligatures
LIGATURES = [0xFB00, 0xFB01, 0xFB02, 0xFB03, 0xFB04]
# FontAwesome PUA range
PUA_RANGE = range(0xF000, 0xFA00)


def compute_scale(face, cell_h):
    """Compute the pixel size to use for a given cell height, matching ab_glyph's logic."""
    ft = _load_freetype()
    # Set to cell_h first to measure natural height
    ft.FT_Set_Pixel_Sizes(face, 0, cell_h)
    metrics = face.contents.size.contents.metrics
    ascender = metrics.ascender / 64.0
    descender = metrics.descender / 64.0
    natural_h = ascender - descender
    if natural_h <= 0:
        return cell_h
    return round((cell_h / natural_h) * cell_h)


def render_atlas(ft_ctx, font_specs, extra_cps, cell_h_hint):
    """Render an atlas from a font chain.

    font_specs: [("path.ttf", size), ...]
    extra_cps: [int, ...] — additional Unicode code points
    cell_h_hint: from first font spec's size field

    Returns: (cell_h, ascii_adv, ascii_off, ext_entries, alpha_buf)
    """
    faces = []
    pixel_sizes = []
    for path, size in font_specs:
        face = ft_ctx.load_face(path)
        ps = compute_scale(face, size)
        ft_ctx.lib.FT_Set_Pixel_Sizes(face, 0, ps)
        faces.append(face)
        pixel_sizes.append(ps)

    cell_h = cell_h_hint  # Use the first font's requested size as cell height

    # Collect all code points: ASCII + extra + ligatures + PUA scan
    codepoints = list(range(128))
    for cp in extra_cps:
        if cp not in codepoints:
            codepoints.append(cp)
    for cp in LIGATURES:
        if cp not in codepoints:
            codepoints.append(cp)
    # Scan PUA range in each font
    for face in faces:
        for cp in PUA_RANGE:
            if ft_ctx.get_char_index(face, cp) != 0 and cp not in codepoints:
                codepoints.append(cp)

    # For each codepoint, find the first font with a glyph and get advance
    class GlyphInfo:
        __slots__ = ('cp', 'render_cp', 'face_idx', 'adv')
        def __init__(self, cp, render_cp, face_idx, adv):
            self.cp = cp
            self.render_cp = render_cp
            self.face_idx = face_idx
            self.adv = adv

    glyphs = []
    for cp in codepoints:
        candidates = [cp]
        if cp in FALLBACKS:
            candidates.append(FALLBACKS[cp])

        found = False
        for try_cp in candidates:
            if found:
                break
            for fi, face in enumerate(faces):
                # Restore pixel size (may have been changed by compute_scale)
                ft_ctx.lib.FT_Set_Pixel_Sizes(face, 0, pixel_sizes[fi])
                glyph_idx = ft_ctx.get_char_index(face, try_cp)
                if glyph_idx == 0:
                    continue
                ft_ctx.load_glyph(face, glyph_idx)
                slot = face.contents.glyph.contents
                adv = (slot.metrics.horiAdvance + 32) // 64  # 26.6 fixed point → pixels
                if adv > 0:
                    glyphs.append(GlyphInfo(cp, try_cp, fi, adv))
                    found = True
                    break

    # Render all glyphs into a flat alpha buffer
    total = sum(g.adv * cell_h for g in glyphs)
    alpha_buf = bytearray(total)
    ascii_adv = [0] * 128
    ascii_off = [0] * 128
    ext_entries = []  # [(codepoint, offset, advance)]
    ptr = 0

    for g in glyphs:
        face = faces[g.face_idx]
        ft_ctx.lib.FT_Set_Pixel_Sizes(face, 0, pixel_sizes[g.face_idx])
        glyph_idx = ft_ctx.get_char_index(face, g.render_cp)
        ft_ctx.load_glyph(face, glyph_idx)
        slot = face.contents.glyph.contents
        bmp = slot.bitmap

        if g.cp < 128:
            ascii_adv[g.cp] = g.adv
            ascii_off[g.cp] = ptr
        else:
            ext_entries.append((g.cp, ptr, g.adv))

        # Copy bitmap into alpha buffer
        bx = max(0, slot.bitmap_left)
        # Compute baseline from face metrics
        metrics = face.contents.size.contents.metrics
        ascender_px = (metrics.ascender + 32) // 64
        by = max(0, ascender_px - slot.bitmap_top)

        for row in range(bmp.rows):
            dy = by + row
            if dy >= cell_h:
                break
            for col in range(bmp.width):
                dx = bx + col
                if dx >= g.adv:
                    break
                a = bmp.buffer[row * bmp.pitch + col]
                alpha_buf[ptr + dy * g.adv + dx] = a

        ptr += g.adv * cell_h

    return cell_h, ascii_adv, ascii_off, ext_entries, bytes(alpha_buf)


def emit_binary_and_rust(atlases, out_dir, out=sys.stdout):
    """Write binary alpha files and Rust source with metadata.

    Binary files: {out_dir}/{atlas_id}.alpha
    Rust source: written to stdout, references the binary files via include_bytes!
    """
    import os
    os.makedirs(out_dir, exist_ok=True)

    out.write("// Auto-generated font atlas metadata. Do not edit.\n")
    out.write("// Re-run gen_font_atlas.py to update.\n\n")

    for atlas_id, (cell_h, ascii_adv, ascii_off, ext, alpha) in atlases:
        uid = atlas_id.upper()
        # Write binary alpha data
        alpha_path = os.path.join(out_dir, f"{atlas_id}.alpha")
        with open(alpha_path, "wb") as f:
            f.write(alpha)

        out.write(f"pub const {uid}_CELL_H: usize = {cell_h};\n")
        out.write(f"pub const {uid}_ASCII_ADV: [usize; 128] = {list(ascii_adv)};\n")
        out.write(f"pub const {uid}_ASCII_OFF: [usize; 128] = {list(ascii_off)};\n")

        if ext:
            entries = ", ".join(f"({cp}, {off}, {adv})" for cp, off, adv in ext)
            out.write(f"pub const {uid}_EXT: &[(u32, usize, usize)] = &[{entries}];\n")
        else:
            out.write(f"pub const {uid}_EXT: &[(u32, usize, usize)] = &[];\n")

        # Reference binary file via include_bytes!
        out.write(f'pub const {uid}_ALPHA: &[u8] = include_bytes!("{alpha_path}");\n')
        out.write("\n")


def main():
    # Args: output_dir (for binary files), JSON spec on stdin
    if len(sys.argv) < 2:
        print("Usage: gen_font_atlas.py <output_dir> < spec.json", file=sys.stderr)
        sys.exit(1)
    out_dir = sys.argv[1]

    specs = json.loads(sys.stdin.read())
    ft = FreeType()
    atlases = []
    for spec in specs:
        atlas_id = spec["id"]
        font_specs = [(f[0], f[1]) for f in spec["fonts"]]
        extra = spec.get("extra", [])
        cell_h = font_specs[0][1]
        result = render_atlas(ft, font_specs, extra, cell_h)
        atlases.append((atlas_id, result))
        print(f"  {atlas_id}: {len(result[4])} bytes, {cell_h}px", file=sys.stderr)
    emit_binary_and_rust(atlases, out_dir)


if __name__ == "__main__":
    main()
