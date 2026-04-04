struct GuiState {
    menu: ui::Menu,
    quit: bool,
    t:    f32,
    pending_active: Option<usize>,
}

impl ui::Callbacks for GuiState {
    fn quit(&mut self) { self.quit = true; }
    fn nav(&mut self, args: &[&str]) {
        if let Some(m) = ui::to_menu(args.first().copied().unwrap_or("")) {
            self.menu = m;
        }
    }
    fn click (&mut self)              { println!("click"); }
    fn action(&mut self, args: &[&str]) {
        self.pending_active = match args.first().copied() {
            Some("a") => Some(0), Some("b") => Some(1), Some("c") => Some(2), _ => None,
        };
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

fn main() {
    let mut backend = init_backend(ui::SCR_W, ui::SCR_H);
    let mut state = GuiState { menu: ui::Menu::Hello, quit: false, t: 0.0, pending_active: None };
    let mut router = ui::Router::new();

    while !state.quit {
        let raw = current_values(state.t);
        let changes: Vec<(&str, &str)> = raw.iter().map(|(n, v)| (*n, v.as_str())).collect();

        router.update_events(backend.poll_events(), &mut state);
        if let Some(sel) = state.pending_active.take() {
            let btns = &["Option A", "Option B", "Option C"];
            for (i, btn) in btns.iter().enumerate() {
                router.set_active(btn, i == sel);
            }
        }
        router.update_menu(&mut *backend, state.menu);
        router.update_params(&mut *backend, &changes);

        state.t += 0.033;
        imgrids::sleep(33);
    }
}
