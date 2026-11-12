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
fn missing_vcruntime_fails_the_import_check() {
    let app = env_file("APP");
    let guard = dx_test_scratch::scratch("msvc-imports");
    let scratch = guard.path();
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
