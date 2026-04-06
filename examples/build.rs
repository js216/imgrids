use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=../demo.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/layout.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/symbols.lua");

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ui_dir = PathBuf::from(&crate_dir).join("src/ui");

    let input = std::fs::File::open("../demo.lua").expect("demo.lua");
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

    // Generate prebaked Myriad atlas for raw example
    println!("cargo:rerun-if-changed=../../imgrids/src/fonts/MyriadPro-Regular.ttf");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/gen_font_atlas.py");
    let font_cache = PathBuf::from(&crate_dir).join("../../target/font_cache").canonicalize()
        .unwrap_or_else(|_| {
            let p = PathBuf::from(&crate_dir).join("../../target/font_cache");
            std::fs::create_dir_all(&p).unwrap();
            p.canonicalize().unwrap()
        });
    let spec = r#"[{"id": "raw_value", "fonts": [["imgrids/src/fonts/MyriadPro-Regular.ttf", 53]], "extra": []}]"#;
    let gen_out = Command::new("python3")
        .arg("imgrids/transpiler/gen_font_atlas.py")
        .arg(&font_cache)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .current_dir("../../")
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(spec.as_bytes()).unwrap();
            child.wait_with_output()
        })
        .expect("gen_font_atlas.py failed to run");
    if !gen_out.status.success() {
        panic!("gen_font_atlas.py failed");
    }
    let raw_fonts_rs = out_dir.join("raw_fonts.rs");
    std::fs::write(&raw_fonts_rs, &gen_out.stdout).unwrap();

    // Copy .html files (if any) next to the final binary output.
    // OUT_DIR is target/<triple>/release/build/<crate>-<hash>/out/
    // Binaries land in target/<triple>/release/
    if let Some(bin_dir) = out_dir.ancestors().nth(3) {
        for entry in std::fs::read_dir(".").unwrap().flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "html").unwrap_or(false) {
                let _ = std::fs::copy(&path, bin_dir.join(path.file_name().unwrap()));
            }
        }
    }
}
