use imgrids::Rgb565;
pub type Pixel = Rgb565;
pub mod ui;

include!(concat!(env!("OUT_DIR"), "/all_menus.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use imgrids_buf::BufBackend;
    fn test_params() -> Vec<(&'static str, String)> {
        let mut p = vec![
            ("parameter One", "0.500".into()),
            ("parameter Two", "0.750".into()),
            ("parameter Three", "0.300".into()),
            (
                "color_demo",
                "Whi\x01te+Gre\x00en \x02Red\x00\n\x03Small Yellow\x00 Normal".into(),
            ),
        ];
        for i in 1..=9 {
            p.push((
                match i {
                    1 => "cell_1",
                    2 => "cell_2",
                    3 => "cell_3",
                    4 => "cell_4",
                    5 => "cell_5",
                    6 => "cell_6",
                    7 => "cell_7",
                    8 => "cell_8",
                    _ => "cell_9",
                },
                i.to_string(),
            ));
        }
        p
    }

    #[test]
    fn snapshot_all_menus() {
        let mut backend = BufBackend::<Rgb565>::new(ui::SCR_W, ui::SCR_H);
        let mut router = ui::Router::new();
        let params = test_params();
        let refs: Vec<(&str, &str)> = params.iter().map(|(n, v)| (*n, v.as_str())).collect();

        let golden_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden");
        let update = std::env::var("SNAPSHOT_UPDATE").is_ok();
        if update {
            std::fs::create_dir_all(&golden_dir).unwrap();
        }

        let mut failures = Vec::new();

        for &(name, menu) in ALL_MENUS {

            router.force_redraw();
            router.update_menu(&mut backend, menu);
            router.update_params(&mut backend, &refs);

            let rendered = backend.ppm();
            let golden_path = golden_dir.join(format!("{name}.ppm"));

            if update {
                std::fs::write(&golden_path, &rendered).unwrap();
                eprintln!("wrote {}", golden_path.display());
            } else if golden_path.exists() {
                let golden = std::fs::read(&golden_path).unwrap();
                if rendered != golden {
                    failures.push(name.to_owned());
                }
            } else {
                eprintln!(
                    "no golden for {name} — run with SNAPSHOT_UPDATE=1 to create"
                );
            }
        }

        assert!(
            failures.is_empty(),
            "snapshot mismatch: {}",
            failures.join(", ")
        );
    }
}
