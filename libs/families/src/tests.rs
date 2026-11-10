use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[test]
fn shipped_table_parses() {
    assert!(families().iter().any(|family| family.name == "python"));
    assert!(parse("{").is_err());
}

#[test]
fn every_extension_has_one_family() {
    let mut owners: BTreeMap<String, &str> = BTreeMap::new();
    for family in families() {
        for ext in &family.file_extensions {
            let seen = owners.insert(ext.to_ascii_lowercase(), &family.name);
            assert!(seen.is_none(), "{ext} is in {seen:?} and {}", family.name);
        }
    }
}

#[test]
fn extension_lookup_ignores_case() {
    assert_eq!(
        family_for_extension(".PY").map(|family| family.name.as_str()),
        Some("python")
    );
    assert!(family_for_extension(".nope").is_none());
}

#[test]
fn family_without_module_counts_as_installed() {
    let none = BTreeSet::new();
    let expected = families()
        .iter()
        .filter(|family| family.module.is_none())
        .count();
    assert_eq!(installed(&none).len(), expected);
}

#[test]
fn module_field_parses() {
    let text = r#"{"families": {"go": {"module": "rules_dx_go"}, "core": {}}}"#;
    let all = parse(text).unwrap();
    assert_eq!(all[0].name, "core");
    assert_eq!(all[0].module, None);
    assert_eq!(all[1].module.as_deref(), Some("rules_dx_go"));
}

#[test]
fn quality_entries_merge_by_aspect_file() {
    let merged = parse_quality(&[
        r#"{"quality": {"bzl": "quality:families/a.bzl", "file_extensions": [".y"]}}"#,
        r#"{"quality": {"bzl": "quality:families/a.bzl", "file_extensions": [".x"], "aspects": {"lint": ["a_lint"]}, "probes": {"lint": ["a_probe"]}}}"#,
        r#"{"name": "no quality"}"#,
    ])
    .expect("parse");
    assert_eq!(
        merged,
        vec![QualityFiles {
            bzl: "quality:families/a.bzl".to_owned(),
            file_extensions: vec![".x".to_owned(), ".y".to_owned()],
            aspects: BTreeMap::from([("lint".to_owned(), vec!["a_lint".to_owned()])]),
            probes: BTreeMap::from([("lint".to_owned(), vec!["a_probe".to_owned()])]),
        }]
    );
    assert!(parse_quality(&["{"]).is_err());
}

#[test]
fn every_quality_extension_has_one_aspect_file() {
    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for language in quality_languages() {
        assert!(!language.file_extensions.is_empty(), "{}", language.bzl);
        for ext in &language.file_extensions {
            let seen = owners.insert(ext, &language.bzl);
            assert!(seen.is_none(), "{ext} is in {seen:?} and {}", language.bzl);
        }
    }
    assert!(quality_languages().len() > 20);
}

#[test]
fn every_language_aspect_label_is_listed_once() {
    let mut seen = std::collections::BTreeSet::new();
    for language in quality_languages() {
        for command in ["format", "lint", "typecheck"] {
            for probes in [false, true] {
                for label in language.labels(command, probes) {
                    assert!(seen.insert(label.clone()), "{label} is listed twice");
                }
            }
        }
    }
    assert!(seen.len() > 100);
}
