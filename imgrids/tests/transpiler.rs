// Integration tests for the Lua transpiler (transpiler/layout.lua).
//
// Each test feeds a .lua fixture through the transpiler and checks:
//   - exit code (0 = success, 1 = warnings or errors)
//   - stderr against the companion *.stderr file
//
// To update expected output after intentional changes:
//   lua transpiler/layout.lua /tmp/ui_test < tests/transpiler/FOO.lua 2> tests/transpiler/FOO.stderr

use std::fs;
use std::process::{Command, Stdio};

fn run(input: &str) -> (i32, String) {
    let file = fs::File::open(input).unwrap_or_else(|_| panic!("missing fixture: {input}"));
    let tmp = tempfile::tempdir().expect("create temp dir");
    let out = Command::new("lua")
        .arg("transpiler/layout.lua")
        .arg(tmp.path())
        .stdin(Stdio::from(file))
        .output()
        .expect("lua not found — install lua to run transpiler tests");
    let rc     = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (rc, stderr)
}

fn expected_stderr(name: &str) -> String {
    let path = format!("tests/transpiler/{name}.stderr");
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing expected output: {path}"))
}

#[test]
fn warnings_finish_and_exit_nonzero() {
    let (rc, stderr) = run("tests/transpiler/warnings.lua");
    assert_ne!(rc, 0, "warnings.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("warnings"));
}

#[test]
fn error_bad_menu_name_exits_immediately() {
    let (rc, stderr) = run("tests/transpiler/test.lua");
    assert_ne!(rc, 0, "test.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("test"));
}

#[test]
fn error_undefined_color() {
    let (rc, stderr) = run("tests/transpiler/bad_color.lua");
    assert_ne!(rc, 0, "bad_color.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("bad_color"));
}

#[test]
fn error_undefined_font() {
    let (rc, stderr) = run("tests/transpiler/bad_font.lua");
    assert_ne!(rc, 0, "bad_font.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("bad_font"));
}
