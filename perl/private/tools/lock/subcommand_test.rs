use std::process::Command;

#[test]
fn a_manifest_without_a_lock_is_stale() {
    let lock = dx_testing::rlocation(&std::env::var("LOCK").expect("LOCK")).expect("lock launcher");
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("cpanfile"), "requires 'Clone::PP';\n").expect("cpanfile");
    let output = Command::new(lock)
        .arg("--dir")
        .arg(dir.path())
        .arg("--check")
        .output()
        .expect("run lock");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("cpan.lock.json is stale"), "{stderr}");
}
