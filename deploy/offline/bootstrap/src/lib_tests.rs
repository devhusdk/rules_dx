//! Tests for the offline bootstrap helpers.

use std::path::PathBuf;

use super::{host_launcher, manifest_entry, pinned, run, today_utc, usage, Options, ADVISORY_SETS};

fn scratch(name: &str) -> PathBuf {
    dx_testing::mkscratch(name).unwrap_or_else(|error| panic!("test scratch: {error}"))
}

#[test]
fn every_host_maps_to_the_launcher_its_bundle_carries() {
    assert_eq!(
        host_launcher("linux", "x86_64"),
        Some("bazelisk-linux-amd64")
    );
    assert_eq!(
        host_launcher("linux", "aarch64"),
        Some("bazelisk-linux-arm64")
    );
    assert_eq!(
        host_launcher("macos", "x86_64"),
        Some("bazelisk-darwin-amd64")
    );
    assert_eq!(
        host_launcher("macos", "aarch64"),
        Some("bazelisk-darwin-arm64")
    );
    assert_eq!(
        host_launcher("windows", "x86_64"),
        Some("bazelisk-windows-amd64.exe")
    );
    assert_eq!(host_launcher("freebsd", "x86_64"), None);
    assert_eq!(host_launcher("windows", "aarch64"), None);
}

#[test]
fn the_curated_set_list_stays_pinned() {
    assert_eq!(
        ADVISORY_SETS,
        ["cargo", "npm", "maven", "nuget", "go", "rubygems"]
    );
}

#[test]
fn a_manifest_line_splits_into_a_digest_and_a_name() {
    assert_eq!(
        manifest_entry("abc123  cargo.json"),
        Some(("abc123".to_string(), "cargo.json".to_string()))
    );
    assert_eq!(
        manifest_entry("  abc123   cargo.json  "),
        Some(("abc123".to_string(), "cargo.json".to_string()))
    );
}

#[test]
fn blank_and_comment_manifest_lines_name_nothing() {
    assert_eq!(manifest_entry(""), None);
    assert_eq!(manifest_entry("   "), None);
    assert_eq!(manifest_entry("# nothing pinned"), None);
    assert_eq!(manifest_entry("  # indented comment"), None);
}

#[test]
fn a_manifest_without_a_name_still_reports_the_digest() {
    assert_eq!(
        manifest_entry("abc123"),
        Some(("abc123".to_string(), String::new()))
    );
}

#[test]
fn a_pinned_digest_is_found_by_file_name() {
    let manifest = "aaa  one.json\nbbb  two.json\nccc  three.json\n";
    assert_eq!(pinned(manifest, "two.json").as_deref(), Some("bbb"));
    assert_eq!(pinned(manifest, "missing.json"), None);
}

#[test]
fn the_last_manifest_line_is_read_without_a_trailing_newline() {
    let manifest = "aaa  one.json\nbbb  two.json";
    assert_eq!(pinned(manifest, "two.json").as_deref(), Some("bbb"));
}

#[test]
fn today_is_a_plain_utc_date() {
    let today = today_utc();
    assert_eq!(today.len(), 10, "today looks like YYYY-MM-DD: {today}");
    assert_eq!(
        today
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '-')
            .count(),
        10,
        "today carries only digits and dashes: {today}"
    );
}

#[test]
fn a_missing_bundle_is_refused_before_anything_else() {
    let options = Options {
        bundle: None,
        install_dir: "/tmp/install".to_string(),
        workspace: "/tmp/workspace".to_string(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        "missing --bundle DIR (vendored offline bundle)"
    );
    let empty = Options {
        bundle: Some(String::new()),
        ..options
    };
    assert_eq!(
        run(&empty, "linux", "x86_64", "2026-01-01"),
        "missing --bundle DIR (vendored offline bundle)"
    );
}

#[test]
fn a_bundle_dir_that_is_absent_is_refused() {
    let options = Options {
        bundle: Some("/nonexistent/offline/bundle".to_string()),
        install_dir: "/tmp/install".to_string(),
        workspace: "/tmp/workspace".to_string(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        "bundle dir does not exist: /nonexistent/offline/bundle"
    );
}

#[test]
fn a_bundle_without_the_two_bundle_dirs_is_refused() {
    let root = scratch("dx-bootstrap-shape-");
    let options = Options {
        bundle: Some(root.to_string_lossy().into_owned()),
        install_dir: root.join("bin").to_string_lossy().into_owned(),
        workspace: root.to_string_lossy().into_owned(),
    };
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        format!("bundle has no bazelisk/ dir: {}", root.to_string_lossy())
    );
    std::fs::create_dir(root.join("bazelisk")).expect("bazelisk dir");
    assert_eq!(
        run(&options, "linux", "x86_64", "2026-01-01"),
        format!("bundle has no advisory/ dir: {}", root.to_string_lossy())
    );
}

#[test]
fn an_unsupported_host_is_refused() {
    let root = scratch("dx-bootstrap-host-");
    std::fs::create_dir(root.join("bazelisk")).expect("bazelisk dir");
    std::fs::create_dir(root.join("advisory")).expect("advisory dir");
    let options = Options {
        bundle: Some(root.to_string_lossy().into_owned()),
        install_dir: root.join("bin").to_string_lossy().into_owned(),
        workspace: root.to_string_lossy().into_owned(),
    };
    assert_eq!(
        run(&options, "plan9", "risc-v", "2026-01-01"),
        "unsupported host plan9-risc-v"
    );
}

#[test]
fn the_usage_line_names_the_required_bundle() {
    assert!(usage().contains("--bundle DIR"));
}
