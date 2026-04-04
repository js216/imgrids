struct GuiState {
    menu: ui::Menu,
    quit: bool,
    t:    f32,
    pending_active: Option<usize>,
    pending_grid: Option<String>,
    nav_changed: bool,
}

impl ui::Callbacks for GuiState {
    fn quit(&mut self) { self.quit = true; }
    fn nav(&mut self, args: &[&str]) {
        if let Some(m) = ui::to_menu(args.first().copied().unwrap_or("")) {
            self.menu = m;
            self.nav_changed = true;
        }
    }
    fn click (&mut self)              { println!("click"); }
    fn action(&mut self, args: &[&str]) {
        self.pending_active = match args.first().copied() {
            Some("a") => Some(0), Some("b") => Some(1), Some("c") => Some(2), _ => None,
        };
    }
    fn select_digit(&mut self, args: &[&str]) {
        self.pending_grid = args.first().map(|s| s.to_string());
    }
}

fn current_values(t: f32) -> Vec<(&'static str, String)> {
    vec![
        ("parameter One", format!("{:.3}", t.sin().abs())),
        ("parameter Two", format!("{:.3}", (t * 0.5).cos().abs() * 1.3)),
        ("parameter Three", format!("{:.3}", (t * 0.3).sin())),
        ("color_demo", "Whi\x01te+Gre\x00en \x02Red\x00\n\x03Small Yellow\x00 Normal".to_owned()),
    ]
}

fn grid_values() -> Vec<(&'static str, String)> {
    (1..=9).map(|i| {
        let name: &'static str = match i {
            1 => "cell_1", 2 => "cell_2", 3 => "cell_3",
            4 => "cell_4", 5 => "cell_5", 6 => "cell_6",
            7 => "cell_7", 8 => "cell_8", _ => "cell_9",
        };
        (name, i.to_string())
    }).collect()
}

fn main() {
    let mut backend = init_backend(ui::SCR_W, ui::SCR_H);
    let mut state = GuiState {
        menu: ui::Menu::Hello, quit: false, t: 0.0,
        pending_active: None, pending_grid: None, nav_changed: false,
    };
    let mut router = ui::Router::new();

    // Initialize grid: set cell values and pre-focus center cell
    let grid = grid_values();
    let grid_refs: Vec<(&str, &str)> = grid.iter().map(|(n, v)| (*n, v.as_str())).collect();
    router.update_menu(&mut *backend, ui::Menu::Grid);
    router.update_params(&mut *backend, &grid_refs);
    router.set_focused("cell_5");
    router.force_redraw();

    while !state.quit {
        let mut raw = current_values(state.t);
        raw.extend(grid_values());
        let changes: Vec<(&str, &str)> = raw.iter().map(|(n, v)| (*n, v.as_str())).collect();

        router.update_events(backend.poll_events(), &mut state);
        if let Some(sel) = state.pending_active.take() {
            let btns = &["Option A", "Option B", "Option C"];
            for (i, btn) in btns.iter().enumerate() {
                router.set_active(btn, i == sel);
            }
        }
        if state.nav_changed {
            state.nav_changed = false;
            router.clear_all_active();
            // Save and clear focus state on menu switch
            let _prev = router.get_focused();
            router.set_focused_raw(None);
            router.clear_focused();
        }
        if let Some(cell_num) = state.pending_grid.take() {
            let (px, right) = router.last_press();
            let label = format!("cell_{cell_num}");
            if let Some((bx, by, bw, bh)) = router.label_bounds(&label) {
                println!("grid: cell {cell_num} pressed at x={px} (right={right}), bounds=({bx},{by} {bw}x{bh}), focused={:?}",
                    router.get_focused());
            }
        }
        router.update_menu(&mut *backend, state.menu);
        router.update_params(&mut *backend, &changes);

        state.t += 0.033;
        imgrids::sleep(33);
    }
}
