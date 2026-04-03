use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=../demo.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/layout.lua");
    println!("cargo:rerun-if-changed=../../imgrids/transpiler/symbols.lua");
    println!("cargo:rerun-if-changed=../../target/font_cache");

    let input = std::fs::File::open("../demo.lua").expect("demo.lua");
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
