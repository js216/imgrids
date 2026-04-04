# API update: per-menu file splitting

## Module include change

```rust
// Before:
#[allow(dead_code)] mod ui { include!(concat!(env!("OUT_DIR"), "/ui.rs")); }

// After:
mod ui;
```

Remove `#[allow(dead_code)]`. The generated code no longer produces
dead-code warnings.

The transpiler now writes to `src/ui/` so Rust's module system finds
the files natively. No `include!` needed.

Add `src/ui/` to your `.gitignore` — these are generated files.

## build.rs change

The transpiler now takes an output directory as its first argument and
writes files directly into `src/ui/` instead of writing to stdout.

```rust
// Before:
let out = Command::new("lua")
    .arg("imgrids/transpiler/layout.lua")
    .stdin(input)
    .current_dir("../../")
    .output()
    .expect("lua not found");
if !out.status.success() {
    panic!("transpiler failed:\n{}", String::from_utf8_lossy(&out.stderr));
}
std::fs::write(out_dir.join("ui.rs"), &out.stdout).expect("write ui.rs");

// After:
let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
let ui_dir = PathBuf::from(&crate_dir).join("src/ui");

let status = Command::new("lua")
    .arg("imgrids/transpiler/layout.lua")
    .arg(&ui_dir)
    .stdin(input)
    .current_dir("../../")
    .status()
    .expect("lua not found");
if !status.success() {
    panic!("transpiler failed (check stderr above)");
}
```

## Conditionally generated methods

The following Router methods are now only generated when the menu
definition uses the corresponding feature. If your app calls one of
these and it no longer compiles, your menu definition is missing the
feature that requires it.

| Method | Generated when |
|---|---|
| `set_focused(label)` | any menu has focusable elements with labels |
| `clear_focused()` | same |
| `get_focused()` | same |
| `set_focused_raw(idx)` | same |
| `focused_adjust()` | any focusable element has `adjust` |
| `label_bounds(label)` | any dynamic element has a label |
| `format_param(name, val)` | any parameter has formatting metadata |
| `FMT_PARAMS` | same |

Methods that were already conditional (unchanged):
`set_active`, `clear_all_active`, `init_auto_active`.

Methods that are always generated (unchanged):
`new`, `update_events`, `update_menu`, `update_params`,
`force_redraw`, `last_press`.

## Public API unchanged

`Router`, `Menu`, `Callbacks`, `to_menu()`, `SCR_W`, `SCR_H` remain
the same. No changes to method signatures or behavior.
