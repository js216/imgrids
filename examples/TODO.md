# TODO: Zero-Overhead GUI System Implementation

## 1. Overview

The goal is to implement a compiler that transforms the GUI DSL into a static,
flattened array of rendering primitives. The runtime should then iterate this
array in a "main loop" with zero layout calculations, using the provided
`lib.rs` for hardware-accelerated rendering and input.

Rewrite `layout.rs` and `demo.rs` to implement the code described in this
document. `layout.rs` does all the parsing and "GUI compiling", while `demo.rs`
needs to be very clean: the absolute minimum of boilerplate code possible!

## 2. Compiler Implementation (Lua to Intermediate Representation)

**Goal:** Parse `ui.lua` and resolve all inherited attributes and geometry.

- [ ] **Lua Environment Setup:**
    - Use `mlua` or `hlua` to load `ui.lua`.
    - Provide a "pre-pass" to resolve `defaults` and global tables (`colors`,
      `fonts`).
- [ ] **Recursive Layout Solver:**
    - Input: A nested table (e.g., `complex_menu`).
    - Logic:
        - Start with `screen` dimensions.
        - Calculate `flex` sizing: For `row`/`col`, sum the `weight` of
          children.
        - Subtract `margin`, `border.width`, and `pad` (in pixels) from
          available space.
        - Position children using absolute `(x, y)` coordinates based on `align`
          and `anchor`.
- [ ] **Font-Color Pre-Baking:**
    - The `lib.rs` requires `Pixel` values. Use the `rgb!` macro to convert `{R,
      G, B}` from `ui.lua` into the target bit-depth (`u16` or `u32`).
    - Group every unique `(font_path, size, fg, bg)` triplet. The compiler must
      ensure these specific rasterized versions exist before runtime.

## 3. Data Structures (Rust)

**Goal:** Define the "Compiled" format stored in memory.

- [ ] **Render Primitive:**
  ```rust
  pub enum RenderOp {
      FillRect { x: usize, y: usize, w: usize, h: usize, color: Pixel },
      DrawBorder { x: usize, y: usize, w: usize, h: usize, thickness: usize, color: Pixel, side: Option<BorderSide> },
      StaticText { x: usize, y: usize, text: String, font_id: usize },
      DynamicText { x: usize, y: usize, cache_idx: usize, font_id: usize },
      Widget { x: usize, y: usize, w: usize, h: usize, kind: String, cache_idx: usize },
  }
  ```

- [ ] Input Hitbox:
  ```rust
  pub struct Hitbox {
      x: usize, y: usize, w: usize, h: usize,
      on_press: String, // Name of the callback to trigger
  }
  ```

## 4. Main Loop & Runtime Logic

**Goal:** High-speed execution using the Push-Value model.

- [ ] **Initialization:**
    - Initialize the `Backend` (SDL, Framebuffer, or Web).
    - Initialize the `Connection` to the parameter server (cursor = 0).
    - Load the compiled `RenderOp` list for the initial menu.
- [ ] **The "Sync" Phase:**
    - Follow the logic in **Section 6.3**: fetch updates via cursor, filter
      against the active menu's reverse index, and update the string cache.
- [ ] **The "Render" Phase:**
    - Call `Backend::render(|pixels, stride| { ... })`.
    - Loop through the `RenderOp` list.
    - If an element is Dirty or a full-redraw is flagged:
        - Use `backend.fill_rect` for backgrounds/borders.
        - Use `renderer.draw` for text (pulling from the String Cache).
- [ ] **The "Input" Phase:**
    - Call `backend.poll_events()`.
    - For `InputEvent::Press { x, y }`, iterate the active `Hitbox` list.
    - If `(x, y)` is within a hitbox, trigger `handle_callback(on_press)`.

## 5. Optimization Checklist

- [ ] **Integer Math:** Ensure all layout calculations in the compiler result in
  integers to avoid sub-pixel jitter in the runtime.
- [ ] **Dirty-Bit Skipping:** In the render loop, if a dynamic label hasn't
  changed, skip the `renderer.draw` call to save CPU/Bus cycles.
- [ ] **Compile-time Warnings:** Emit a log if `border.width == 0` but a `side`
  is specified, or if a `weight` is 0.

## 6. Data Binding, Callbacks, and the "Push-Value" Sync Phase

To achieve true zero-overhead, the system utilizes a "Push-Value" model.
The runtime leverages a cursor-based update stream to receive only
the parameters that have changed since the last poll, along with their
new f64 values.

### 6.1 The Compiler's Role (Menu Isolation)

When the compiler processes the `menus` table from `ui.lua`:
* **Scoped Baking:** It generates a unique, flattened `RenderOp` array and
  a unique `Reverse Index` for **each** top-level menu key.
* **Reverse Index:** Maps a parameter key (e.g., `"temp"`) to a list of
  indices within that specific menu's array.

### 6.2 The Designer's Role (The Bridge)

The GUI designer provides only the formatting and interaction logic:
1. **`fn format_value(key: &str, val: f64) -> String`**: Converts the
   f64 value provided by the update stream into a display string.
2. **`fn handle_callback(name: &str)`**: Logic executed for `press` events.

### 6.3 The "Visibility-Filtered" Sync Logic (Main Loop)

The runtime maintains a `current_menu` pointer and a persistent 64-bit
cursor. Every frame:
1. **Dirty Check:** Request updates from the parameter server using the
   current cursor. The server returns a list of `(String, f64)`
   pairs.
2. **Active Filter:** For each `(key, new_val)` in the results:
   - Perform a lookup in the `current_menu.reverse_index`.
   - **If Key is Missing:** Skip immediately. Updates for background
     menus consume zero CPU cycles for formatting or cache writes.
3. **Targeted Update:** If the key exists in the active menu:
   - Call `format_value(key, new_val)`.
   - Update the **String Cache** for all affected `RenderOp` indices.
   - Set the **Dirty Flag** for those specific Ops.
4. **Cursor Commit:** Update the local cursor to the latest value
   provided by the server to mark these changes as processed.

### 6.4 The Interaction Contract

- **Hitbox Scope:** The runtime only checks the `Hitbox` list associated
  with the `current_menu`.
- **Event Dispatch:** On a touch/click, the AABB check is performed
  exclusively against visible elements.
- **Execution:** The runtime passes the `press` string directly to the
  designer's `handle_callback` function.

# 7. Style Resolution and Inheritance

**Goal:** Ensure that all visual attributes are fully resolved at compile time
so that the runtime does not perform any style lookups or inheritance logic.

## 7.1 Style Sources

An element’s final style is determined by merging attributes from the following
sources, in order of increasing priority:

1. Global defaults
2. Screen defaults
3. Menu-level attributes
4. Parent element attributes
5. Element’s own attributes

Later sources override earlier ones.

### Example Resolution Order

If a font is specified at the menu level and a background color is specified on
a row, and a label specifies only a foreground color, the final label style will
be:

- Font → from menu-level attribute
- Background → from row
- Foreground → from label itself

## 7.2 Compiler Behavior

- During compilation, each element is converted into a fully-resolved `Style`
  struct containing all attributes (`font`, `fg`, `bg`, `padding`, `margin`,
  `border`, `align`, `weight`, `act`, etc.).
- The runtime consumes only these resolved styles; no further inheritance or
  lookups occur at runtime.
- Warnings are emitted if attributes are inconsistent, e.g., `border.width == 0`
  but a side is specified, or `weight == 0`.

# 8. Menu Switching

**Goal:** Allow fast switching between top-level menus with zero layout
calculations at runtime.

## 8.1 Compiled Menu Structure

Each top-level menu key in `ui.lua` generates a unique `CompiledMenu` struct:

```rust
struct CompiledMenu {
    render_ops: Vec<RenderOp>,
    hitboxes: Vec<Hitbox>,
    reverse_index: HashMap<String, Vec<usize>>, // maps parameter keys to RenderOp indices
}
```

## 8.2 Runtime Switching

The runtime holds a pointer to the current_menu and its associated render_ops
and hitboxes.  To switch menus:

- Swap the current_menu pointer
- Optionally flag a full redraw
- Reset any menu-specific state (e.g., string cache entries)
- Only hitboxes for the active menu are checked for input events.

# 9. String Cache Structure

**Goal:** Minimize runtime overhead for dynamic text rendering.

## 9.1 Cache Layout

- Pre-allocated array of `StringCacheEntry` structs:

```rust
struct StringCacheEntry {
    text: String,
        dirty: bool,
        }
```

Each RenderOp::DynamicText points to its string in the cache via cache_idx.

## 9.2 Cache Update

During the sync phase, only values present in the current_menu.reverse_index are updated.
If a value has not changed, its dirty flag remains false and the render loop skips drawing.
The runtime never formats numbers or text for invisible menus.

# 10. Compiler Passes

**Goal:** Transform the nested GUI DSL into a flattened, zero-overhead runtime
structure.

## 10.1 Proposed Pass Sequence

1. **Load Lua tables** (`ui.lua`)
2. **Resolve global defaults** (`colors`, `fonts`)
3. **Build intermediate layout tree** from nested tables
4. **Resolve style inheritance** (merge defaults, menu, parent, element)
5. **Calculate layout sizes** based on weights, padding, and margins
6. **Calculate absolute positions** for each element
7. **Generate RenderOps** from resolved layout and style
8. **Generate Hitboxes** for interactive elements
9. **Generate Reverse Index** mapping parameter keys to RenderOp indices
10. **Bake fonts and pre-rasterize text** for all unique `(font, fg, bg)` combinations
