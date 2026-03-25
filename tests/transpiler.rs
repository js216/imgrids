// Integration tests for the Lua transpiler (scripts/layout.lua).
//
// Each test feeds a .lua fixture through the transpiler and checks:
//   - exit code (0 = success, 1 = warnings or errors)
//   - stderr against the companion *.stderr file
//
// To update expected output after intentional changes:
//   lua scripts/layout.lua < tests/transpiler/FOO.lua > /dev/null 2> tests/transpiler/FOO.stderr

use std::fs;
use std::process::{Command, Stdio};

fn run(input: &str) -> (i32, String, String) {
    let file = fs::File::open(input).unwrap_or_else(|_| panic!("missing fixture: {input}"));
    let out = Command::new("lua")
        .arg("scripts/layout.lua")
        .stdin(Stdio::from(file))
        .output()
        .expect("lua not found — install lua to run transpiler tests");
    let rc     = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (rc, stdout, stderr)
}

fn expected_stderr(name: &str) -> String {
    let path = format!("tests/transpiler/{name}.stderr");
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing expected output: {path}"))
}

#[test]
fn warnings_finish_and_exit_nonzero() {
    let (rc, _stdout, stderr) = run("tests/transpiler/warnings.lua");
    assert_ne!(rc, 0, "warnings.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("warnings"));
}

#[test]
fn error_bad_menu_name_exits_immediately() {
    let (rc, _stdout, stderr) = run("tests/transpiler/test.lua");
    assert_ne!(rc, 0, "test.lua must exit nonzero");
    assert_eq!(stderr, expected_stderr("test"));
}
