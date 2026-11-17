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
            file_names: Vec::new(),
            aspects: BTreeMap::from([("lint".to_owned(), vec!["a_lint".to_owned()])]),
            probes: BTreeMap::from([("lint".to_owned(), vec!["a_probe".to_owned()])]),
        }]
    );
    assert!(parse_quality(&["{"]).is_err());
}

#[test]
fn file_name_entry_is_wanted_by_its_file_name() {
    let merged = parse_quality(&[
        r#"{"quality": {"bzl": "a:a.bzl", "file_extensions": [".x"], "file_names": ["Xfile", "Xfile"]}}"#,
    ])
    .expect("parse");
    assert_eq!(merged[0].file_names, vec!["Xfile".to_owned()]);
    let named = std::collections::BTreeSet::from(["Xfile".to_owned()]);
    assert!(merged[0].wanted(&named));
    assert!(!merged[0].wanted(&std::collections::BTreeSet::new()));
}

#[test]
fn every_quality_language_is_wanted_by_a_file() {
    let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for language in quality_languages() {
        assert!(!language.file_extensions.is_empty(), "{}", language.bzl);
        for ext in &language.file_extensions {
            owners.entry(ext).or_default().push(&language.bzl);
        }
    }
    let yaml = owners.get(".yaml").cloned().unwrap_or_default();
    for (ext, files) in &owners {
        assert!(
            files.len() == 1 || files.iter().all(|file| yaml.contains(file)),
            "{ext} is in {files:?}"
        );
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

#[test]
fn licenses_list_the_setup_ids_with_the_quality_marks() {
    let cc = licenses()
        .iter()
        .find(|entry| entry.language == "cc")
        .expect("cc lists a license");
    assert!(cc.marks.iter().any(|mark| mark == ".cc"), "{cc:?}");
    assert!(
        cc.licenses
            .iter()
            .any(|license| license.id == "visual-studio-build-tools"
                && license.platforms.iter().any(|p| p == "windows_x86_64")),
        "{cc:?}"
    );
    assert!(licenses().iter().all(|entry| !entry.licenses.is_empty()));
}

#[test]
fn setup_flags_follow_the_named_languages() {
    let flags = setup_flags(["python", "rust"]);
    assert_eq!(
        flags,
        vec![(
            "python".to_owned(),
            vec!["common --modify_execution_info=PySdist.*=+block-network".to_owned()]
        )]
    );
    assert_eq!(
        setup_flags(["rust"]),
        [] as [(std::string::String, std::vec::Vec<std::string::String>); 0]
    );
}
