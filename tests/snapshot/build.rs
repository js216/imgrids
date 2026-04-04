use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../examples/demo.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/layout.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/symbols.lua");
    println!("cargo:rerun-if-changed=../../target/font_cache");

    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ui_dir = crate_dir.join("src/ui");

    let input = std::fs::File::open("../../examples/demo.lua").expect("demo.lua");
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

    // Extract menu variant names from the generated enum and write a helper.
    let mod_rs = std::fs::read_to_string(ui_dir.join("mod.rs")).unwrap();
    let tag = "pub enum Menu {";
    let enum_start = mod_rs.find(tag).expect("Menu enum not found") + tag.len();
    let enum_end = enum_start + mod_rs[enum_start..].find('}').unwrap();
    let variants: Vec<&str> = mod_rs[enum_start..enum_end]
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut gen = String::from("pub const ALL_MENUS: &[(&str, ui::Menu)] = &[\n");
    for v in &variants {
        gen.push_str(&format!("    (\"{v}\", ui::Menu::{v}),\n"));
    }
    gen.push_str("];\n");
    std::fs::write(out_dir.join("all_menus.rs"), gen).unwrap();
}
