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
    fn function_cl(&mut self)              { println!("function_cl"); }
    fn function_pr(&mut self, args: &[&str]) { println!("function_pr {:?}", args); }
    fn fn_multi   (&mut self, args: &[&str]) { println!("fn_multi {:?}", args); }
    fn fn3        (&mut self)              { println!("fn3"); }
    fn click      (&mut self)              { println!("click"); }
}

fn current_values(t: f32) -> [(&'static str, String); 2] {
    [
        ("parameter One", format!("{:.3}", t.sin())),
        ("parameter Two", format!("{:.3}", (t * 0.5).cos().abs())),
    ]
}

fn main() {
    let mut backend = imgrids::init(800, 480);
    let mut state = GuiState { menu: ui::Menu::Simple, quit: false, t: 0.0 };

    loop {
        let raw = current_values(state.t);
        let changes: Vec<(&str, &str)> = raw.iter().map(|(n, v)| (*n, v.as_str())).collect();

        ui::update_events(backend.poll_events(), &mut state);
        ui::update_menu(&mut *backend, state.menu);
        ui::update_changes(&mut *backend, &changes);

        if state.quit { return; }
        state.t += 0.033;
        imgrids::sleep(33);
    }
}
