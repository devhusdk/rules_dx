use std::process::Command;

#[test]
fn a_manifest_without_a_lock_is_stale() {
    let lock = dx_testing::rlocation(&std::env::var("LOCK").expect("LOCK")).expect("lock launcher");
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Package.swift"),
        "// swift-tools-version:5.9\nimport PackageDescription\n\nlet package = Package(name: \"app\")\n",
    )
    .expect("Package.swift");
    let output = Command::new(lock)
        .arg("--check")
        .arg(format!("--dir={}", dir.path().display()))
        .output()
        .expect("run lock");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("swiftpm.lock.json is stale"), "{stderr}");
}
