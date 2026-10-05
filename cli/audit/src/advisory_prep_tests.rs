use std::path::Path;

use super::*;
use crate::advisory::{advisory_source, is_audit_date, parse_identity, Freshness};
use crate::vuln::parse_snapshot;

const CRATES_URL: &str = "https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip";

fn entry(name: &str, text: &str) -> (String, Vec<u8>) {
    (name.to_owned(), text.as_bytes().to_vec())
}

fn advisory_json(ecosystem: &str, id: &str, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","modified":"2026-01-01T00:00:00Z","affected":[{{"package":{{"ecosystem":"{ecosystem}","name":"{name}"}},"versions":["1.0.0"]}}]}}"#
    )
}

fn cargo_advisory(id: &str, name: &str) -> String {
    advisory_json("crates.io", id, name)
}

fn entries() -> Vec<(String, Vec<u8>)> {
    vec![
        entry("all/9/GHSA-demo.json", &cargo_advisory("GHSA-demo", "demo")),
        entry(
            "all/2/GHSA-other.json",
            &format!(
                "[{},{}]",
                cargo_advisory("GHSA-a", "aaa"),
                cargo_advisory("GHSA-b", "bbb")
            ),
        ),
    ]
}

#[test]
fn entries_merge_into_one_snapshot_the_audit_can_read() {
    let prepared =
        convert_entries("cargo", CRATES_URL, "2026-09-22", &entries()).expect("converts");
    assert_eq!(prepared.advisories, 3);
    assert_eq!(prepared.snapshot.set, "cargo");
    assert_eq!(
        prepared.snapshot.url,
        advisory_source("cargo").expect("source")
    );
    assert_eq!(prepared.snapshot.retrieved_at, "2026-09-22");
    assert_eq!(prepared.snapshot.path, ".dx/advisory/cargo.json");
    assert_eq!(
        prepared.snapshot.sha256,
        dx_digest::sha256_hex(&prepared.payload)
    );
    let text = String::from_utf8(prepared.payload.clone()).expect("utf-8");
    let advisories = parse_snapshot(&text).expect("the audit reads the payload");
    assert_eq!(advisories.len(), 3);
    assert_eq!(advisories[0].id, "GHSA-a");
    assert_eq!(advisories[2].id, "GHSA-demo");
    assert!(
        advisories.iter().all(|found| found.set == "cargo"),
        "{advisories:?}"
    );
}

#[test]
fn the_snapshot_bytes_do_not_depend_on_the_archive_entry_order() {
    let forward = convert_entries("cargo", CRATES_URL, "2026-09-22", &entries()).expect("converts");
    let mut reversed = entries();
    reversed.reverse();
    let backward = convert_entries("cargo", CRATES_URL, "2026-09-22", &reversed).expect("converts");
    assert_eq!(forward.payload, backward.payload);
    assert_eq!(forward.snapshot, backward.snapshot);
    assert_eq!(forward.snapshot.sha256, backward.snapshot.sha256);
}

#[test]
fn the_sidecar_round_trips_and_binds_the_payload_bytes() {
    let prepared =
        convert_entries("cargo", CRATES_URL, "2026-09-22", &entries()).expect("converts");
    let bytes = identity_bytes(&prepared.snapshot).expect("sidecar");
    let text = String::from_utf8(bytes).expect("utf-8");
    assert!(text.ends_with('\n'), "{text}");
    assert!(text.contains("\n  \"sha256\": "), "{text}");
    assert_eq!(
        parse_identity(&text).expect("the audit reads the sidecar"),
        prepared.snapshot
    );
    assert!(validate_prepared(&prepared.snapshot, &prepared.payload).is_ok());
    assert_eq!(
        validate_prepared(&prepared.snapshot, b"[]"),
        Err(PrepError::BadIdentity {
            family: "cargo".to_owned(),
            detail: "sha256 does not match .dx/advisory/cargo.json".to_owned()
        })
    );
}

#[test]
fn every_broken_entry_is_reported_with_its_name() {
    let json_cases = ["{", "", "[{\"id\":}]"];
    for body in json_cases {
        let error = convert_entries(
            "cargo",
            CRATES_URL,
            "2026-09-22",
            &[entry("all/1.json", body)],
        )
        .expect_err("broken JSON");
        assert!(
            matches!(error, PrepError::BadEntryJson { .. }),
            "{body:?}: {error}"
        );
        assert!(error.to_string().contains("all/1.json"), "{error}");
        assert!(
            error.to_string().starts_with(&format!("{PREP_CODE}:")),
            "{error}"
        );
    }
    for body in ["42", "\"text\"", "null", "true"] {
        let error = convert_entries(
            "cargo",
            CRATES_URL,
            "2026-09-22",
            &[entry("all/1.json", body)],
        )
        .expect_err("not an advisory");
        assert!(
            matches!(error, PrepError::BadEntry { .. }),
            "{body:?}: {error}"
        );
        assert!(error.to_string().contains("all/1.json"), "{error}");
    }
}

#[test]
fn an_archive_without_advisories_is_refused() {
    assert_eq!(
        convert_entries("cargo", CRATES_URL, "2026-09-22", &[]),
        Err(PrepError::NoJsonEntries {
            family: "cargo".to_owned()
        })
    );
    assert_eq!(
        convert_entries(
            "cargo",
            CRATES_URL,
            "2026-09-22",
            &[entry("all/1.json", "[]")]
        ),
        Err(PrepError::NoAdvisories {
            family: "cargo".to_owned()
        })
    );
}

#[test]
fn a_malformed_retrieval_date_is_refused_before_any_merge() {
    for date in ["2026-9-22", "2026/09/22", "not-a-date", "2026-13-01", ""] {
        assert!(!is_audit_date(date), "{date} is not an audit date");
        assert_eq!(
            convert_entries("cargo", CRATES_URL, date, &entries()),
            Err(PrepError::BadDate {
                retrieved_at: date.to_owned()
            }),
            "{date}"
        );
    }
}

#[test]
fn a_prepared_snapshot_is_judged_fresh_the_same_way_the_audit_judges_it() {
    let prepared =
        convert_entries("cargo", CRATES_URL, "2026-09-22", &entries()).expect("converts");
    assert_eq!(
        advisory::freshness(&prepared.snapshot, "2026-09-22"),
        Freshness::Fresh
    );
    assert_eq!(
        advisory::freshness(&prepared.snapshot, "2026-09-23"),
        Freshness::Stale
    );
}

#[test]
fn one_fetch_is_bounded_and_pinned_to_https() {
    let plan = fetch_plan(CRATES_URL, Path::new("/tmp/dx/all.zip"), DEFAULT_LIMITS).expect("plans");
    assert_eq!(plan.program, "curl");
    assert_eq!(
        plan.argv,
        [
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https,file",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "30",
            "--max-time",
            "600",
            "--retry",
            "3",
            "--retry-delay",
            "5",
            "--output",
            "/tmp/dx/all.zip",
            CRATES_URL,
        ]
        .map(str::to_owned)
        .to_vec()
    );
    assert_eq!(DEFAULT_LIMITS.connect_timeout_seconds, 30);
    assert_eq!(DEFAULT_LIMITS.max_seconds, 600);
    assert_eq!(DEFAULT_LIMITS.retries, 3);
    assert_eq!(DEFAULT_LIMITS.retry_delay_seconds, 5);
}

#[test]
fn a_fetch_refuses_a_url_or_limit_it_cannot_hold_to() {
    for url in [
        "http://osv.dev/all.zip",
        "ftp://osv.dev/all.zip",
        "not a url",
        "",
    ] {
        assert_eq!(
            fetch_plan(url, Path::new("/tmp/dx/all.zip"), DEFAULT_LIMITS),
            Err(PrepError::BadUrl {
                url: url.to_owned()
            }),
            "{url}"
        );
    }
    assert!(fetch_plan(
        "file:///opt/dx/all.zip",
        Path::new("/tmp/dx/all.zip"),
        DEFAULT_LIMITS
    )
    .is_ok());
    for limits in [
        FetchLimits {
            connect_timeout_seconds: 0,
            ..DEFAULT_LIMITS
        },
        FetchLimits {
            max_seconds: 30,
            ..DEFAULT_LIMITS
        },
        FetchLimits {
            max_seconds: 30,
            connect_timeout_seconds: 30,
            ..DEFAULT_LIMITS
        },
        FetchLimits {
            retry_delay_seconds: 0,
            ..DEFAULT_LIMITS
        },
    ] {
        assert_eq!(
            fetch_plan(CRATES_URL, Path::new("/tmp/dx/all.zip"), limits),
            Err(PrepError::BadLimits { limits }),
            "{limits:?}"
        );
    }
}

fn workspace_with(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp workspace");
    for rel in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("creates");
        std::fs::write(&path, b"lock").expect("writes");
    }
    dir
}

fn families_of(dir: &Path) -> Vec<&'static str> {
    needed_families(dir).expect("reads the workspace")
}

#[test]
fn only_the_ecosystems_the_dependency_sets_need_are_prepared() {
    let bare = workspace_with(&["src/lib.rs", "third_party"]);
    assert_eq!(families_of(bare.path()), Vec::<&str>::new());
    let npm = workspace_with(&["pnpm-lock.yaml"]);
    assert_eq!(families_of(npm.path()), vec!["npm"]);
    let npm_family = workspace_with(&["quality/tools/javascript/pnpm-lock.yaml"]);
    assert_eq!(
        families_of(npm_family.path()),
        vec!["npm"],
        "every npm set shares the npm snapshot"
    );
    let one_of_many = workspace_with(&["package-lock.json"]);
    assert_eq!(families_of(one_of_many.path()), vec!["npm"]);
    let shared = workspace_with(&["pnpm-lock.yaml", "third_party/dotnet/paket.lock"]);
    assert_eq!(families_of(shared.path()), vec!["npm", "nuget"]);
    let ruby = workspace_with(&[
        "third_party/ruby/Gemfile.lock",
        "examples/adopt-ruby/Gemfile.lock",
    ]);
    assert_eq!(families_of(ruby.path()), vec!["rubygems"]);
    let powershell = workspace_with(&["third_party/powershell/PSGallery.lock.json"]);
    assert_eq!(families_of(powershell.path()), vec!["nuget"]);
    let go = workspace_with(&["third_party/go/go.mod"]);
    assert_eq!(families_of(go.path()), vec!["go"]);
}

#[test]
fn every_audited_set_names_locks_and_one_advisory_family() {
    let mut families: Vec<&str> = Vec::new();
    for set in AUDITED_SETS {
        assert!(!is_empty_set(set), "{set} needs lock coverage");
        let family = advisory::advisory_family(set).unwrap_or_else(|| panic!("{set}"));
        families.push(family);
    }
    families.sort_unstable();
    families.dedup();
    assert_eq!(
        families,
        ["cargo", "go", "maven", "npm", "nuget", "rubygems"]
    );
    let workspace = workspace_with(&vuln_locks("cargo"));
    assert_eq!(
        families_of(workspace.path()),
        vec!["cargo"],
        "one cargo lock asks for the cargo snapshot alone"
    );
}
