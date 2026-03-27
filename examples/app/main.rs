mod ui;

struct GuiState {
    menu: ui::Menu,
    quit: bool,
    t:    f32,
}

impl ui::Callbacks for GuiState {
    fn quit(&mut self) { self.quit = true; }
    fn nav(&mut self, args: &[&str]) {
        if let Some(m) = ui::to_menu(args.first().copied().unwrap_or("")) {
            println!("{}", args[0]);
            self.menu = m;
        }
    }
    fn click (&mut self)              { println!("click"); }
    fn action(&mut self, args: &[&str]) { println!("action {:?}", args); }
}

fn current_values(t: f32) -> [(&'static str, String); 2] {
    [
        ("parameter One", format!("{:.3}", t.sin().abs())),
        ("parameter Two", format!("{:.3}", (t * 0.5).cos().abs())),
    ]
}

fn main() {
    let mut backend = imgrids::init(ui::SCR_W, ui::SCR_H);
    let mut state = GuiState { menu: ui::Menu::Hello, quit: false, t: 0.0 };
    ui::force_redraw();

    while !state.quit {
        let raw = current_values(state.t);
        let changes: Vec<(&str, &str)> = raw.iter().map(|(n, v)| (*n, v.as_str())).collect();

        ui::update_events(backend.poll_events(), &mut state);
        ui::update_menu(&mut *backend, state.menu);
        ui::update_params(&mut *backend, &changes);

        state.t += 0.033;
        imgrids::sleep(33);
    }
}
