use dx_testing::env_file;
use std::process::Command;

fn archiver(scratch: &std::path::Path, args: &[&str]) -> std::process::Output {
    let tool = env_file("ARCHIVER");
    Command::new(tool)
        .arg("--imports")
        .arg(scratch.join("ok"))
        .args(args)
        .output()
        .expect("run archiver")
}

#[test]
fn missing_libstdcxx_member_fails_the_import_check() {
    let app = env_file("APP");
    let guard = dx_test_scratch::scratch("gcc-imports");
    let scratch = guard.path();
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
    let app = env_file("APP");
    let guard = dx_test_scratch::scratch("gcc-imports-ok");
    let scratch = guard.path();
    let member = format!("hello_cxx_gcc={}", app.display());
    let out = archiver(&scratch, &["--provides", "libstdc++.so.6", &member]);
    assert!(out.status.success(), "failed: {out:?}");
}
