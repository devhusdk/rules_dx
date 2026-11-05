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
fn missing_vcruntime_fails_the_import_check() {
    let app = resolve_runfiles(&std::env::var("APP").expect("APP"));
    let scratch = mkscratch("msvc-imports").expect("scratch");
    let member = format!("hello_msvc.exe={}", app.display());
    let out = archiver(&scratch, &["--check", "hello_msvc.exe", &member]);
    assert!(!out.status.success(), "unexpected success: {out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("vcruntime140.dll"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("not shipped and not a system import"),
        "stderr: {stderr}"
    );
}
