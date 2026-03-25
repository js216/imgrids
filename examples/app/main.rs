// Minimal app showing how to use the transpiled ui module.
//
// ui.rs is the stub (replace it with transpiler output to see the real GUI).
// This file stays unchanged regardless of what ui.lua contains.

mod ui;

use imgrids::InputEvent;

// ---- Press callbacks -------------------------------------------------
//
// One function per unique "press" name in ui.lua.
// Signature must be: fn name(args: &[&str]) -> Option<ui::Menu>
// The generated update() calls these as super::name(&["arg1", ...]).

fn function_cl(args: &[&str]) -> Option<ui::Menu> { let _ = args; Some(ui::Menu::Simple)    }
fn function_pr(args: &[&str]) -> Option<ui::Menu> { let _ = args; Some(ui::Menu::DynStat)   }
fn fn_multi   (args: &[&str]) -> Option<ui::Menu> { let _ = args; None                      }
fn fn3        (args: &[&str]) -> Option<ui::Menu> { let _ = args; Some(ui::Menu::Grid)      }
fn click      (args: &[&str]) -> Option<ui::Menu> { let _ = args; Some(ui::Menu::Clickable) }

// ---- Fake data -------------------------------------------------------
//
// Stand-in for whatever source provides parameter values in a real app.
// Returns the dynamic labels that ui.lua's menus reference.

fn current_values(t: f32) -> [(&'static str, String); 2] {
    [
        ("parameter One", format!("{:.3}", t.sin())),
        ("parameter Two", format!("{:.3}", (t * 0.5).cos().abs())),
    ]
}

// ---- Main loop -------------------------------------------------------

fn main() {
    let mut backend = imgrids::init(800, 480);

    let mut menu = ui::Menu::Clickable;
    ui::draw(&mut *backend, menu);

    let mut t = 0.0f32;

    loop {
        // Copy events before mutably borrowing backend for update().
        let events: Vec<InputEvent> = backend.poll_events().to_vec();
        if events.iter().any(|e| matches!(e, InputEvent::Quit)) {
            return;
        }

        // Build the changes slice for this frame.
        let raw = current_values(t);
        let changes: Vec<(&str, &str)> = raw.iter()
            .map(|(n, v)| (*n, v.as_str()))
            .collect();

        if let Some(next) = ui::update(&mut *backend, menu, &changes, &events) {
            menu = next;
            ui::draw(&mut *backend, menu);
        }

        t += 0.033;
        imgrids::sleep(33);
    }
}
