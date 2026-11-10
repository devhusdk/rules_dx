use dx_testing::resolve_runfiles;
use std::process::{Command, Output};

fn run(metadata: &str, args: &[&str]) -> Output {
    let dir = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR");
    let root = std::path::Path::new(&dir).join("bcr");
    let module = root.join("modules/rules_dx");
    std::fs::create_dir_all(&module).expect("mkdir");
    std::fs::write(module.join("metadata.json"), metadata).expect("write metadata");
    Command::new("bash")
        .arg(resolve_runfiles("tools/ci/upgrade_e2e.sh"))
        .args(args)
        .arg("--resolve-only")
        .env("BCR_URL", format!("file://{}", root.display()))
        .env_remove("GITHUB_EVENT_NAME")
        .output()
        .expect("spawn upgrade_e2e.sh")
}

fn resolved(metadata: &str, args: &[&str]) -> String {
    let out = run(metadata, args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("utf8").trim().to_string()
}

const META: &str = r#"{"versions":["1.0.0","1.1.0","1.2.0","1.3.0"],"yanked_versions":{"1.3.0":"bad"}}"#;

#[test]
fn no_arguments_use_the_last_two_unyanked_versions() {
    assert_eq!(resolved(META, &[]), "1.1.0 1.2.0");
}

#[test]
fn a_lone_from_resolves_to_from_the_bcr() {
    assert_eq!(resolved(META, &["--from", "1.0.0"]), "1.0.0 1.2.0");
}

#[test]
fn a_lone_to_resolves_from_from_the_bcr() {
    assert_eq!(resolved(META, &["--to", "local"]), "1.1.0 local");
}

#[test]
fn both_inputs_skip_the_bcr() {
    assert_eq!(
        resolved("not json", &["--from", "1.0.0", "--to", "1.1.0"]),
        "1.0.0 1.1.0"
    );
}

#[test]
fn empty_values_resolve_from_the_bcr() {
    assert_eq!(resolved(META, &["--from", "", "--to", ""]), "1.1.0 1.2.0");
}

#[test]
fn other_arguments_fail() {
    for args in [&["1.0.0", "1.1.0"][..], &["--bogus"]] {
        let out = run(META, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn release_cell_script_parses() {
    let status = Command::new("bash")
        .arg("-n")
        .arg(resolve_runfiles("tools/ci/release_cell.sh"))
        .status()
        .expect("spawn bash");
    assert!(status.success());
}
