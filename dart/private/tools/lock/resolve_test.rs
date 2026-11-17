//! Tests the pub lock step with the pinned Dart SDK on a set with no hosted packages.

use std::path::{Path, PathBuf};

use pub_lock::{Outcome, Request, lock};

const PUBSPEC: &str = "name: app\nenvironment:\n  sdk: ^3.0.0\n";

fn dart() -> PathBuf {
    dx_testing::env_file("DX_DART")
}

fn set(name: &str) -> PathBuf {
    let dir = dx_testing::scratch_dir(name);
    dx_testing::write(&dir.join("pubspec.yaml"), PUBSPEC);
    dir
}

fn request(dir: &Path, check: bool) -> Request {
    Request {
        dir: dir.to_path_buf(),
        manifest: "pubspec.yaml".to_owned(),
        lock: "pubspec.lock".to_owned(),
        check,
    }
}

#[test]
fn a_missing_lock_is_written_then_fresh() {
    let dir = set("written");
    assert_eq!(lock(&dart(), &request(&dir, false)), Ok(Outcome::Written));
    let text = dx_testing::read(&dir.join("pubspec.lock"));
    assert!(text.contains("packages: {}"), "{text}");
    assert!(text.contains("sdks:"), "{text}");
    assert_eq!(lock(&dart(), &request(&dir, false)), Ok(Outcome::Fresh));
    assert_eq!(lock(&dart(), &request(&dir, true)), Ok(Outcome::Fresh));
    assert_eq!(dx_testing::read(&dir.join("pubspec.lock")), text);
    assert!(!dir.join(".dart_tool").exists());
}

#[test]
fn check_reports_a_missing_or_edited_lock_and_writes_nothing() {
    let dir = set("check");
    assert_eq!(lock(&dart(), &request(&dir, true)), Ok(Outcome::Stale));
    assert!(!dir.join("pubspec.lock").exists());
    assert_eq!(lock(&dart(), &request(&dir, false)), Ok(Outcome::Written));
    let edited = format!("{}# edited\n", dx_testing::read(&dir.join("pubspec.lock")));
    dx_testing::write(&dir.join("pubspec.lock"), &edited);
    assert_eq!(lock(&dart(), &request(&dir, true)), Ok(Outcome::Stale));
    assert_eq!(dx_testing::read(&dir.join("pubspec.lock")), edited);
}

#[test]
fn an_unresolvable_manifest_fails_with_the_pub_output() {
    let dir = set("unresolvable");
    dx_testing::write(
        &dir.join("pubspec.yaml"),
        "name: app\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  missing:\n    path: ../nowhere\n",
    );
    let error = lock(&dart(), &request(&dir, false)).expect_err("unresolvable");
    assert!(error.contains("dart pub get failed"), "{error}");
    assert!(!dir.join("pubspec.lock").exists());
}
