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

#[test]
fn quality_marks_list_each_quality_language_with_its_files() {
    let marks = quality_marks();
    assert_eq!(marks.len(), quality_languages_by_name());
    let dockerfile = marks
        .iter()
        .find(|entry| entry.language == "dockerfile")
        .expect("dockerfile has a quality entry");
    assert!(dockerfile.marks.iter().any(|mark| mark == ".dockerfile"));
    assert!(dockerfile.marks.iter().any(|mark| mark == "Dockerfile"));
    assert!(marks.iter().all(|entry| !entry.marks.is_empty()));
}

fn quality_languages_by_name() -> usize {
    shipped()
        .iter()
        .filter(|manifest| manifest.quality.is_some())
        .count()
}

#[test]
fn command_labels_come_from_the_language_commands() {
    assert_eq!(
        command_label("go", "go").as_deref(),
        Some("@rules_dx//go/private/tools:go")
    );
    assert_eq!(command_label("go", "gofmt"), None);
    assert_eq!(command_label("no_such_language", "go"), None);
    assert!(language_commands("no_such_language").is_empty());
    assert_eq!(COSIGN_LABEL, "@rules_dx//deploy/private/release:cosign");
    assert_eq!(DX_LABEL, "@rules_dx//:dx");
    assert!(COMMAND_TARGETS.contains(&GENERATE));
}

/// The text of a shipped source file, before its inline `#[cfg(test)] mod ... {`.
fn shipped_text(text: &str) -> &str {
    text.split_once("\n#[cfg(test)]\n")
        .filter(|(_, rest)| {
            rest.lines()
                .next()
                .is_some_and(|line| line.starts_with("mod ") && line.ends_with('{'))
        })
        .map_or(text, |(shipped, _)| shipped)
}

fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                rust_files(&path, found);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_shipped_source_spells_a_command_label_inline() {
    let mut labels: Vec<String> = COMMAND_TARGETS
        .iter()
        .map(|target| format!("\"@rules_dx//{target}\""))
        .collect();
    for manifest in shipped() {
        labels.extend(
            manifest
                .commands
                .iter()
                .map(|target| format!("\"@rules_dx//{target}\"")),
        );
    }
    assert!(
        labels
            .iter()
            .any(|label| label.contains("go/private/tools:go"))
    );
    let checkout = dx_testing::rules_dx_source().expect("rules_dx checkout");
    let mut files = Vec::new();
    for crate_dir in [
        "cli/update/src",
        "cli/cli/src",
        "cli/audit/src",
        "cli/exec_audit/src",
    ] {
        rust_files(&checkout.join(crate_dir), &mut files);
    }
    assert!(!files.is_empty(), "no source under {}", checkout.display());
    let mut inline = Vec::new();
    for file in files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        if name.is_some_and(|name| {
            name == "tests.rs" || name.ends_with("_tests.rs") || name.ends_with("_test.rs")
        }) {
            continue;
        }
        let text = std::fs::read_to_string(&file).expect("read source");
        for label in &labels {
            if shipped_text(&text).contains(label.as_str()) {
                inline.push(format!("{}: {label}", file.display()));
            }
        }
    }
    assert_eq!(
        inline,
        Vec::<String>::new(),
        "read these labels from dx_families"
    );
}
