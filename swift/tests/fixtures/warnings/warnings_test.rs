#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]
use std::path::{Path, PathBuf};
use std::process::Command;

const WARNING: &str = "initialization of immutable value 'unused'";

fn compile(swift: &str, source: &Path, module_cache: &Path, flags: &[&str]) -> (bool, String) {
    // Without -module-cache-path clang caches modules under $HOME/.cache, and
    // HOME is unwritable (/nonexistent) on remote execution.
    let output = Command::new(swift)
        .arg("--driver-mode=swiftc")
        .arg("-typecheck")
        .arg("-module-cache-path")
        .arg(module_cache)
        .args(flags)
        .arg(source)
        .output()
        .expect("run swift");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

#[test]
fn warnings_as_errors_fails_a_swift_warning() {
    let swift = std::env::var("DX_SWIFT").expect("DX_SWIFT");
    let fixture = std::env::var("DX_FIXTURE").expect("DX_FIXTURE");
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR"));
    let source = tmp.join("warn.swift");
    let module_cache = tmp.join("module-cache");
    std::fs::copy(&fixture, &source).expect("copy the fixture");
    let (ok, plain) = compile(&swift, &source, &module_cache, &[]);
    assert!(ok, "the warning alone failed the compile\n{plain}");
    assert!(plain.contains(&format!("warning: {WARNING}")), "{plain}");
    let (ok, strict) = compile(&swift, &source, &module_cache, &["-warnings-as-errors"]);
    assert!(!ok, "-warnings-as-errors compiled a warning\n{strict}");
    assert!(strict.contains(&format!("error: {WARNING}")), "{strict}");
}
