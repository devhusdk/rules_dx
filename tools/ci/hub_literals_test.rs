use std::path::Path;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use starlark_syntax as _;
use yaml_serde as _;

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

fn non_test(source: &str) -> &str {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if line.trim_start().starts_with("#[cfg(test)]") {
            return source.get(..offset).expect("string slice within bounds");
        }
        offset += line.len();
    }
    source
}

fn binds_hub(before: &str) -> bool {
    let before = before.trim_end();
    let before = before.strip_suffix("Some(").unwrap_or(before).trim_end();
    let Some(target) = before
        .strip_suffix(":=")
        .or_else(|| before.strip_suffix("=="))
        .or_else(|| before.strip_suffix('='))
        .or_else(|| before.strip_suffix(':'))
    else {
        return false;
    };
    let start = target
        .rfind(['(', ',', '{', '[', ';'])
        .map_or(0, |index| index + 1);
    target
        .get(start..)
        .expect("string slice within bounds")
        .to_ascii_lowercase()
        .contains("hub")
}

fn violations(
    hubs: &BTreeSet<String>,
    rust: &BTreeMap<String, String>,
    lock_scan: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let go = lock_scan.iter().filter(|(path, _)| Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("go")));
    let mut out = BTreeSet::new();
    for (path, source) in rust.iter().chain(go) {
        for (index, line) in non_test(source).lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for hub in hubs {
                let literal = format!("\"{hub}\"");
                for (at, _) in line.match_indices(literal.as_str()) {
                    if binds_hub(line.get(..at).expect("string slice within bounds")) {
                        out.insert(format!("{path}:{} {hub}", index + 1));
                    }
                }
            }
        }
    }
    out
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

const RUST_LANG: &str =
    r#"{"name": "rust", "setup": {"hubs": [{"ecosystem": "cargo", "hub": "crates"}]}}"#;
const GO_LANG: &str = r#"{"name": "go", "setup": {"hubs": [{"ecosystem": "go", "hub": "*"}]}}"#;

#[test]
fn the_tree_has_no_bare_hub_literal() {
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
        .filter(|path| Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("go")))
        .count();
    assert!(
        go >= MIN_GO,
        "only {go} non-test .go files in the lock scan snapshot"
    );
    let found = violations(&hubs(&files), &rust, &lock_scan);
    assert!(
        found.is_empty(),
        "bare hub literals; read the hub name from the lock registry:\n{}",
        found.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn hubs_skip_the_wildcard_hub() {
    let files = texts(&[("rust/lang.json", RUST_LANG), ("go/lang.json", GO_LANG)]);
    assert_eq!(hubs(&files), lines(&["crates"]));
}

#[test]
fn a_hub_constant_is_a_violation() {
    let hubs = lines(&["crates", "npm"]);
    let rust = texts(&[(
        "cli/a.rs",
        "const DEFAULT_HUB: &str = \"crates\";\nlet x = Spec { hub: Some(\"npm\".to_owned()) };\n",
    )]);
    let go = texts(&[("gazelle/a.go", "package a\n\nvar defaultHub = \"npm\"\n")]);
    assert_eq!(
        violations(&hubs, &rust, &go),
        lines(&["cli/a.rs:1 crates", "cli/a.rs:2 npm", "gazelle/a.go:3 npm"])
    );
}

#[test]
fn ecosystem_keys_and_test_modules_pass() {
    let hubs = lines(&["crates", "maven"]);
    let rust = texts(&[(
        "cli/a.rs",
        "let hub = dx_families::hub(\"maven\");\nlet c = lock.get(\"crates\");\n// DEFAULT_HUB = \"crates\"\n#[cfg(test)]\nmod tests {\n    const HUB: &str = \"crates\";\n}\n",
    )]);
    let go = texts(&[(
        "gazelle/a.go",
        "DefaultRepo: registryEntry(\"maven\").Hub,\n",
    )]);
    assert!(violations(&hubs, &rust, &go).is_empty());
}

#[test]
fn a_hub_comparison_is_a_violation() {
    let hubs = lines(&["npm"]);
    let go = texts(&[("gazelle/a.go", "if hub == \"npm\" {\n")]);
    assert_eq!(
        violations(&hubs, &BTreeMap::new(), &go),
        lines(&["gazelle/a.go:1 npm"])
    );
}
