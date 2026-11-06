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
    let report = plan(dir.path(), Some("v1.2.3"), MINOR_LOG, None).expect("plan");
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
    let major = plan(dir.path(), Some("v1.2.3"), MAJOR_LOG, None).expect("plan");
    assert_eq!(major.next.as_deref(), Some("2.0.0"));
    let quiet = plan(dir.path(), Some("v1.2.3"), QUIET_LOG, None).expect("plan");
    assert_eq!(quiet.next, None);
    assert!(quiet.render().contains("next version: none"));
}

#[test]
fn to_flag_overrides_the_derived_one() {
    let dir = workspace(&[("Cargo.toml", "1.2.3")]);
    let report = plan(dir.path(), Some("v1.2.3"), QUIET_LOG, Some("3.0.0")).expect("plan");
    assert_eq!(report.next.as_deref(), Some("3.0.0"));
    assert!(report.render().contains("next version: 3.0.0 (from --to)"));
    let bad = plan(dir.path(), None, "", Some("three"));
    assert!(bad.is_err());
}

#[test]
fn without_a_tag_the_manifests_vote() {
    let dir = workspace(&[
        ("Cargo.toml", "0.4.0"),
        ("a/Cargo.toml", "0.4.0"),
        ("b/Cargo.toml", "0.3.0"),
    ]);
    let report = plan(dir.path(), None, MINOR_LOG, None).expect("plan");
    assert_eq!(report.current.as_deref(), Some("0.4.0"));
    assert_eq!(report.next.as_deref(), Some("0.5.0"));
    assert_eq!(report.out_of_sync(), 1);
}

#[test]
fn apply_writes_the_version_into_every_manifest() {
    let dir = workspace(&[("Cargo.toml", "1.2.3"), ("web/package.json", "1.2.3")]);
    let changed = apply(dir.path(), "1.3.0").expect("apply");
    assert_eq!(changed, ["Cargo.toml", "web/package.json"]);
    let report = plan(dir.path(), Some("v1.3.0"), "", None).expect("plan");
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

#[test]
fn lint_names_non_conventional_subjects() {
    let log = "aaaa1111\0feat: add a thing\0\x1ebbbb2222\0update stuff\0\x1e";
    assert_eq!(
        lint(log),
        [("bbbb2222".to_owned(), "update stuff".to_owned())]
    );
}

#[test]
fn every_log_read_leaves_out_merge_commits() {
    assert!(LOG_ARGS.contains(&"--no-merges"));
}

#[test]
fn no_changelog_file_means_nothing_is_written() {
    let dir = workspace(&[("Cargo.toml", "1.0.0")]);
    let written = write_changelog(dir.path(), "1.1.0", "2026-01-02", MINOR_LOG).expect("write");
    assert!(!written);
    assert!(!dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn an_existing_changelog_gains_the_section() {
    let dir = workspace(&[("Cargo.toml", "1.0.0")]);
    fs::write(dir.path().join("CHANGELOG.md"), "# Changelog\n").expect("write");
    assert!(write_changelog(dir.path(), "1.1.0", "2026-01-02", MINOR_LOG).expect("write"));
    let text = fs::read_to_string(dir.path().join("CHANGELOG.md")).expect("read");
    assert!(
        text.starts_with("# Changelog\n\n## [1.1.0] - 2026-01-02"),
        "{text}"
    );
}

#[test]
fn changelog_section_is_keep_a_changelog_and_goes_above_older_releases() {
    let section = changelog_section("1.3.0", "2026-01-02", MINOR_LOG);
    assert!(
        section.starts_with("## [1.3.0] - 2026-01-02\n\n### "),
        "{section}"
    );
    let fresh = prepend_section("# Changelog\n", &section);
    assert!(fresh.starts_with("# Changelog\n"), "{fresh}");
    assert!(fresh.ends_with(&section), "{fresh}");
    let next = changelog_section("1.4.0", "2026-02-03", MINOR_LOG);
    let both = prepend_section(&fresh, &next);
    let new_at = both.find("## [1.4.0]").expect("new");
    let old_at = both.find("## [1.3.0]").expect("old");
    assert!(new_at < old_at, "{both}");
    assert!(both.starts_with("# Changelog\n"), "{both}");
}

#[test]
fn dates_come_from_days_since_the_epoch() {
    assert_eq!(date_of_day(0), "1970-01-01");
    assert_eq!(date_of_day(20_455), "2026-01-02");
    assert_eq!(date_of_day(19_782), "2024-02-29");
}

#[test]
fn a_breaking_api_report_raises_a_refactor_only_log_to_major() {
    let dir = workspace(&[("Cargo.toml", "1.2.3")]);
    let log = "aaa\0refactor: drop old_fn\0\x1e";
    let mut report = plan(dir.path(), Some("v1.2.3"), log, None).expect("plan");
    assert_eq!(report.next, None);
    assert_eq!(report.major_candidate().as_deref(), Some("2.0.0"));
    let mut asked = Vec::new();
    let labels = "//pkg:lib_config\n//pkg:other_config\n";
    let breaking = breaking_targets(labels, &mut |target| {
        asked.push(target.to_owned());
        Ok(if target == "//pkg:lib" {
            "api compat: 1 breaking changes against 1.2.3\n".to_owned()
        } else {
            "api compat: 0 breaking changes against 1.2.3\n".to_owned()
        })
    })
    .expect("targets");
    assert_eq!(asked, ["//pkg:lib", "//pkg:other"]);
    report.raise_for_api(breaking);
    assert_eq!(report.next.as_deref(), Some("2.0.0"));
    assert_eq!(report.bump, Bump::Major);
    assert!(
        report
            .render()
            .contains("api compat: //pkg:lib reports breaking changes")
    );
}

#[test]
fn skipped_api_checks_do_not_count_as_breaking() {
    assert!(!reports_breaking(
        "api compat: no earlier v<version> tag; skipped (first release)\n"
    ));
    assert!(!reports_breaking(
        "api compat: 0 breaking changes against 1.0.0\n"
    ));
    assert!(reports_breaking(
        "x\napi compat: 2 breaking changes against 1.0.0\n"
    ));
}
