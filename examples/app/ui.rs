// Stub: this file will be replaced by transpiler output.
//   Usage: lua transpiler.lua < examples/ui.lua > examples/app/ui.rs
//
// The transpiler generates one variant per menu in ui.lua.
// Menu names are snake_case in ui.lua; the _menu suffix is stripped and the
// remainder converted to PascalCase for the variant: simple_menu → Simple.
//
// Press callbacks are invoked as super::name(arg) where name and arg come from
// the "press" / "arg" attributes in ui.lua. Every callback must be defined in
// main.rs with the signature:
//   fn name(arg: &str) -> Option<Menu>
// Return Some(next) to switch menus, None to stay on the current one.

use imgrids::{Backend, InputEvent};

#[derive(Clone, Copy, PartialEq)]
pub enum Menu {
    Simple,
    DynStat,
    Widget,
    Grid,
    Popup,
    Unequal,
    Styled,
    SubStyled,
    Clickable,
    Complex,
    Pad,
    Margin,
    Borders,
}

/// Draw the static frame for `menu`.
/// Call once on startup and again whenever the menu changes.
/// Dynamic label cells are left blank; the first update() call populates them.
pub fn draw(_backend: &mut dyn Backend, _menu: Menu) {
    // stub — transpiler emits atlas-blit calls here
}

/// Process one frame.
///
/// `changes`: `(label_name, value)` pairs for dynamic labels whose values
///   changed since the last call. Pass all labels on the first call.
///
/// `events`: slice from backend.poll_events() copied before this call.
///
/// Returns `Some(next)` if a press callback requested a menu switch.
pub fn update(
    _backend: &mut dyn Backend,
    _menu: Menu,
    _changes: &[(&str, &str)],
    _events: &[InputEvent],
) -> Option<Menu> {
    // stub — transpiler emits label redraws and press-callback dispatch here
    None
}
