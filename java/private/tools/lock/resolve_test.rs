//! The maven lock step writes the golden lock from a runfiles repository with an empty `PATH`.

use maven_lock::{Outcome, Request, Tools, config, lock};

const MANIFEST: &str = "[versions]\nlib = \"2.0\"\n\n[libraries]\napp = \"com.example:app:1.0\"\nlib = { module = \"com.example:lib\", version.ref = \"lib\" }\n";

const CANONICAL: &str = "https://repo.example/maven2/";

fn repository() -> String {
    let marker = dx_testing::env_file("REPO");
    let root = marker
        .ancestors()
        .find(|path| path.ends_with("repo"))
        .expect("repository root");
    format!("file:{}/", root.display())
}

fn set_request(dir: &std::path::Path, check: bool) -> Request {
    let hub = dx_families::hub("maven").expect("maven is registered");
    Request {
        dir: dir.to_owned(),
        manifest: hub.manifest.clone(),
        lock: hub.lock.clone().expect("lock"),
        repositories: vec![repository()],
        check,
    }
}

#[test]
fn runfiles_repository_writes_the_golden_lock() {
    let dir = tempfile::tempdir().expect("set directory");
    let request = set_request(dir.path(), false);
    std::fs::write(dir.path().join(&request.manifest), MANIFEST).expect("manifest");
    let tools = Tools::from_env().expect("tools");
    assert_eq!(lock(&tools, &request), Ok(Outcome::Written));
    let lock_path = dir.path().join(&request.lock);
    let written = std::fs::read_to_string(&lock_path).expect("lock");
    let canonical = dir.path().join("canonical.json");
    std::fs::write(&canonical, written.replace(&repository(), CANONICAL)).expect("canonical lock");
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &canonical,
        Some("java/tests/fixtures/maven_lock/golden.json"),
    )
    .expect("the lock matches the golden");
    assert_eq!(lock(&tools, &set_request(dir.path(), true)), Ok(Outcome::Fresh));
}

#[test]
fn version_refs_and_strings_become_artifacts() {
    let rendered = config(MANIFEST, &[]).expect("config");
    let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("json");
    assert_eq!(
        parsed["artifacts"],
        serde_json::json!([
            {"group": "com.example", "artifact": "app", "version": "1.0"},
            {"group": "com.example", "artifact": "lib", "version": "2.0"},
        ])
    );
    assert_eq!(parsed["repositories"], serde_json::json!(["https://repo1.maven.org/maven2/"]));
}

#[test]
fn a_library_without_a_version_fails() {
    let error = config("[libraries]\nx = { module = \"a:b\" }\n", &[]).expect_err("no version");
    assert_eq!(error, "library x has no version");
}
