use std::collections::BTreeMap;

use super::*;

#[test]
fn quality_entries_merge_by_aspect_file() {
    let merged = parse_quality(&[
        r#"{"quality": {"bzl": "core/quality:families/a.bzl", "file_extensions": [".y"]}}"#,
        r#"{"quality": {"bzl": "core/quality:families/a.bzl", "file_extensions": [".x"], "aspects": {"lint": ["a_lint"]}, "probes": {"lint": ["a_probe"]}}}"#,
        r#"{"name": "no quality"}"#,
    ])
    .expect("parse");
    assert_eq!(
        merged,
        vec![QualityFiles {
            bzl: "core/quality:families/a.bzl".to_owned(),
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

#[test]
fn docs_entries_merge_by_aspect_file() {
    let docs = parse_docs(&[
        r#"{"docs": {"bzl": "d:a.bzl", "file_extensions": [".y", ".x"], "aspects": ["a_docs"]}}"#,
        r#"{"docs": {"bzl": "d:a.bzl", "file_extensions": [".x", ".z"], "aspects": []}}"#,
        r#"{"name": "none"}"#,
    ])
    .expect("parses");
    assert_eq!(
        docs,
        vec![DocsFiles {
            bzl: "d:a.bzl".to_owned(),
            file_extensions: vec![".x".to_owned(), ".y".to_owned(), ".z".to_owned()],
            aspects: vec!["a_docs".to_owned()],
        }]
    );
    assert!(parse_docs(&["{"]).is_err());
}

#[test]
fn every_docs_extension_has_one_aspect_file() {
    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for language in docs_languages() {
        assert!(!language.aspects.is_empty(), "{}", language.bzl);
        for ext in &language.file_extensions {
            let seen = owners.insert(ext, &language.bzl);
            assert!(seen.is_none(), "{ext} is in {seen:?} and {}", language.bzl);
        }
    }
    assert!(docs_languages().len() > 20);
}
