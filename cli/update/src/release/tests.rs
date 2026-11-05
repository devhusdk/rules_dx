use super::*;

const MINOR_LOG: &str = include_str!("../../testdata/minor.log");
const MAJOR_LOG: &str = include_str!("../../testdata/major.log");
const QUIET_LOG: &str = include_str!("../../testdata/quiet.log");

fn workspace(versions: &[(&str, &str)]) -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch("release");
    for (path, version) in versions {
        let content = if path.ends_with("package.json") {
            format!("{{\"name\": \"w\", \"version\": \"{version}\"}}\n")
        } else {
            format!("[package]\nname = \"a\"\nversion = \"{version}\"\n")
        };
        dx_test_scratch::write(dir.path(), path, &content);
    }
    dir
}

#[test]
fn feat_since_the_tag_is_a_minor_bump_over_every_manifest() {
    let dir = workspace(&[
        ("Cargo.toml", "1.2.3"),
        ("web/package.json", "1.2.3"),
        ("old/Cargo.toml", "1.1.0"),
    ]);
    let report = plan(
        dir.path(),
        &Config::default(),
        Some("v1.2.3"),
        MINOR_LOG,
        None,
    )
    .expect("plan");
    assert_eq!(report.current.as_deref(), Some("1.2.3"));
    assert_eq!(report.bump, Bump::Minor);
    assert_eq!(report.next.as_deref(), Some("1.3.0"));
    assert_eq!(report.commits, 4);
    let sync: Vec<(&str, bool)> = report
        .manifests
        .iter()
        .map(|line| (line.path.as_str(), line.in_sync))
        .collect();
    assert_eq!(
        sync,
        [
            ("Cargo.toml", true),
            ("old/Cargo.toml", false),
            ("web/package.json", true)
        ]
    );
    assert_eq!(report.out_of_sync(), 1);
}

#[test]
fn breaking_commits_bump_major_and_quiet_logs_bump_nothing() {
    let dir = workspace(&[("Cargo.toml", "1.2.3")]);
    let major = plan(
        dir.path(),
        &Config::default(),
        Some("v1.2.3"),
        MAJOR_LOG,
        None,
    )
    .expect("plan");
    assert_eq!(major.next.as_deref(), Some("2.0.0"));
    let quiet = plan(
        dir.path(),
        &Config::default(),
        Some("v1.2.3"),
        QUIET_LOG,
        None,
    )
    .expect("plan");
    assert_eq!(quiet.next, None);
    assert!(quiet.render().contains("next version: none"));
}

#[test]
fn to_flag_overrides_the_derived_one() {
    let dir = workspace(&[("Cargo.toml", "1.2.3")]);
    let report = plan(
        dir.path(),
        &Config::default(),
        Some("v1.2.3"),
        QUIET_LOG,
        Some("3.0.0"),
    )
    .expect("plan");
    assert_eq!(report.next.as_deref(), Some("3.0.0"));
    assert!(report.render().contains("next version: 3.0.0 (from --to)"));
    let bad = plan(dir.path(), &Config::default(), None, "", Some("three"));
    assert!(bad.is_err());
}

#[test]
fn without_a_tag_the_manifests_vote() {
    let dir = workspace(&[
        ("Cargo.toml", "0.4.0"),
        ("a/Cargo.toml", "0.4.0"),
        ("b/Cargo.toml", "0.3.0"),
    ]);
    let report = plan(dir.path(), &Config::default(), None, MINOR_LOG, None).expect("plan");
    assert_eq!(report.current.as_deref(), Some("0.4.0"));
    assert_eq!(report.next.as_deref(), Some("0.5.0"));
    assert_eq!(report.out_of_sync(), 1);
}

#[test]
fn release_toml_sets_the_tag_prefix() {
    let dir = workspace(&[("Cargo.toml", "1.0.0")]);
    assert_eq!(read_config(dir.path()).expect("default"), Config::default());
    fs::write(dir.path().join("release.toml"), "tag_prefix = \"app-\"\n").expect("write");
    let config = read_config(dir.path()).expect("config");
    assert_eq!(config.tag_prefix, "app-");
    let report = plan(dir.path(), &config, Some("app-1.0.0"), MINOR_LOG, None).expect("plan");
    assert_eq!(report.next.as_deref(), Some("1.1.0"));
    fs::write(dir.path().join("release.toml"), "tag_prefix = 3\n").expect("write");
    assert!(read_config(dir.path()).is_err());
}

#[test]
fn apply_writes_the_version_into_every_manifest() {
    let dir = workspace(&[("Cargo.toml", "1.2.3"), ("web/package.json", "1.2.3")]);
    let changed = apply(dir.path(), "1.3.0").expect("apply");
    assert_eq!(changed, ["Cargo.toml", "web/package.json"]);
    let report = plan(dir.path(), &Config::default(), Some("v1.3.0"), "", None).expect("plan");
    assert_eq!(report.out_of_sync(), 0);
    assert!(report.manifests.iter().all(|line| line.version == "1.3.0"));
}

#[test]
fn tag_message_starts_with_the_tag_and_lists_the_commits() {
    let message = tag_message("v1.3.0", MINOR_LOG);
    assert!(message.starts_with("v1.3.0\n\n"), "{message}");
    assert!(message.contains("## Features"), "{message}");
    assert!(message.contains("## Fixes"), "{message}");
}
