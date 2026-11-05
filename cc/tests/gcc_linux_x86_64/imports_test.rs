use dx_testing::{mkscratch, resolve_runfiles};
use std::process::Command;

fn archiver(scratch: &std::path::Path, args: &[&str]) -> std::process::Output {
    let tool = resolve_runfiles(&std::env::var("ARCHIVER").expect("ARCHIVER"));
    Command::new(tool)
        .arg("--imports")
        .arg(scratch.join("ok"))
        .args(args)
        .output()
        .expect("run archiver")
}

#[test]
fn missing_libstdcxx_member_fails_the_import_check() {
    let app = resolve_runfiles(&std::env::var("APP").expect("APP"));
    let scratch = mkscratch("gcc-imports").expect("scratch");
    let member = format!("hello_cxx_gcc={}", app.display());
    let out = archiver(&scratch, &["--check", "hello_cxx_gcc", &member]);
    assert!(!out.status.success(), "unexpected success: {out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("libstdc++.so.6"), "stderr: {stderr}");
    assert!(
        stderr.contains("not shipped and not a system import"),
        "stderr: {stderr}"
    );
}

#[test]
fn provided_libstdcxx_passes_the_import_check() {
    let app = resolve_runfiles(&std::env::var("APP").expect("APP"));
    let scratch = mkscratch("gcc-imports-ok").expect("scratch");
    let member = format!("hello_cxx_gcc={}", app.display());
    let out = archiver(&scratch, &["--provides", "libstdc++.so.6", &member]);
    assert!(out.status.success(), "failed: {out:?}");
}
