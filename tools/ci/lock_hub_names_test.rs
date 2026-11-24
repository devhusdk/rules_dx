use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_guard::allowlist::{check_lines, line_differences};
use dx_testing::allowlist::parse_lines;
use serde_json::Value;
use starlark_syntax as _;
use yaml_serde as _;

const ALLOWLIST: &str = "tools/ci/lock_hub_names_allowlist.txt";

const MIN_LANGUAGES: usize = 50;
const MIN_RUST: usize = 300;
const MIN_GO: usize = 100;

fn hubs(files: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (path, text) in files {
        if !path.ends_with("/lang.json") {
            continue;
        }
        let lang: Value =
            serde_json::from_str(text).unwrap_or_else(|error| panic!("{path}: {error}"));
        let entries = lang
            .get("setup")
            .and_then(|setup| setup.get("hubs"))
            .and_then(Value::as_array);
        for entry in entries.into_iter().flatten() {
            if let Some(hub) = entry.get("hub").and_then(Value::as_str)
                && !hub.is_empty()
                && hub != "*"
            {
                out.insert(hub.to_owned());
            }
        }
    }
    out
}

fn names_hub(source: &str, hub: &str) -> bool {
    [
        format!("@{hub}//"),
        format!("@{hub}\""),
        format!("\"{hub}//"),
    ]
    .iter()
    .any(|shape| source.contains(shape.as_str()))
}

fn violations(
    hubs: &BTreeSet<String>,
    rust: &BTreeMap<String, String>,
    lock_scan: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let go = lock_scan.iter().filter(|(path, _)| {
        Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("go"))
    });
    let mut out = BTreeSet::new();
    for (path, source) in rust.iter().chain(go) {
        for hub in hubs.iter().filter(|hub| names_hub(source, hub)) {
            out.insert(format!("hub {path} {hub}"));
        }
    }
    out
}

fn well_formed(line: &str) -> bool {
    line.split(' ').count() == 3 && line.starts_with("hub ")
}

fn fix(_line: &str) -> String {
    "read the hub name from the lock registry".to_owned()
}

fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

fn decode_texts(variable: &str) -> BTreeMap<String, String> {
    serde_json::from_str(&read_input(variable))
        .unwrap_or_else(|error| panic!("{variable}: {error}"))
}

fn texts(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

fn lines(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|line| (*line).to_owned()).collect()
}

const PHP_LANG: &str =
    r#"{"name": "php", "setup": {"hubs": [{"ecosystem": "composer", "hub": "composer"}]}}"#;
const GO_LANG: &str = r#"{"name": "go", "setup": {"hubs": [{"ecosystem": "go", "hub": "*"}]}}"#;

#[test]
fn the_tree_matches_the_allowlist() {
    let files = decode_texts("DX_FILES_JSON");
    let rust = decode_texts("DX_RUST_JSON");
    let lock_scan = decode_texts("DX_LOCK_SCAN_JSON");
    let languages = files
        .keys()
        .filter(|path| path.ends_with("/lang.json"))
        .count();
    assert!(
        languages >= MIN_LANGUAGES,
        "only {languages} lang.json files in the files snapshot"
    );
    assert!(
        rust.len() >= MIN_RUST,
        "only {} non-test .rs files in the rust snapshot",
        rust.len()
    );
    let go = lock_scan
        .keys()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("go"))
        })
        .count();
    assert!(
        go >= MIN_GO,
        "only {go} non-test .go files in the lock scan snapshot"
    );
    let problems = check_lines(
        &violations(&hubs(&files), &rust, &lock_scan),
        &read_input("LOCK_HUB_NAMES_ALLOWLIST"),
        well_formed,
        fix,
    );
    assert!(
        problems.is_empty(),
        "{ALLOWLIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
}

#[test]
fn hubs_come_from_setup_hubs_without_the_wildcard() {
    let files = texts(&[
        ("php/lang.json", PHP_LANG),
        ("go/lang.json", GO_LANG),
        ("php/README.md", "@composer//:x"),
    ]);
    assert_eq!(hubs(&files), lines(&["composer"]));
}

#[test]
fn a_hub_label_in_rust_or_go_fails() {
    let hubs = lines(&["composer", "pypi"]);
    let rust = texts(&[
        (
            "cli/a/src/lib.rs",
            "const HUB: &str = \"@composer//:monolog\";\n",
        ),
        (
            "cli/b/src/lib.rs",
            "format!(\"@{}//:x\", registry.hub(\"composer\"))\n",
        ),
    ]);
    let lock_scan = texts(&[
        ("gazelle/x/lang.go", "repo := \"@pypi\"\n"),
        ("gazelle/y/lang.go", "label := \"pypi//:requests\"\n"),
        ("x/module_fragment.bazel", "\"@pypi//:x\"\n"),
    ]);
    assert_eq!(
        violations(&hubs, &rust, &lock_scan),
        lines(&[
            "hub cli/a/src/lib.rs composer",
            "hub gazelle/x/lang.go pypi",
            "hub gazelle/y/lang.go pypi",
        ])
    );
}

#[test]
fn a_longer_repository_name_passes() {
    let hubs = lines(&["npm"]);
    let rust = texts(&[(
        "cli/a/src/lib.rs",
        "\"@npm_typescript//:x\" \"@rules_npm//x\"\n",
    )]);
    assert_eq!(violations(&hubs, &rust, &BTreeMap::new()), BTreeSet::new());
}

#[test]
fn a_new_line_and_a_stale_line_fail() {
    let allowed = lines(&["hub a.rs crates"]);
    assert_eq!(
        line_differences(&allowed, &allowed, fix),
        Vec::<String>::new()
    );
    assert_eq!(
        line_differences(&lines(&["hub a.rs crates", "hub b.go npm"]), &allowed, fix),
        ["new: hub b.go npm; read the hub name from the lock registry"]
    );
    assert_eq!(
        line_differences(&BTreeSet::new(), &allowed, fix),
        ["stale: hub a.rs crates; delete the allowlist line"]
    );
}

#[test]
fn the_allowlist_is_sorted_hub_lines() {
    let (found, problems) = parse_lines("hub a.rs crates\nhub b.go npm\n", well_formed);
    assert_eq!(found.len(), 2);
    assert_eq!(problems, Vec::<String>::new());
    let (_, problems) = parse_lines(
        "hub b.go npm\nhub a.rs crates\n\nname a.rs x\n hub c.rs npm\n",
        well_formed,
    );
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"hub a.rs crates\"",
            "malformed line: \"\"",
            "malformed line: \"name a.rs x\"",
            "malformed line: \" hub c.rs npm\"",
        ]
    );
}
