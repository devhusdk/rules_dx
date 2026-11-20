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
    let unmarked: Vec<&str> = marks
        .iter()
        .filter(|entry| entry.marks.is_empty())
        .map(|entry| entry.language.as_str())
        .collect();
    assert_eq!(unmarked, ["csharp", "cuda", "fsharp"]);
    let manifests = shipped();
    let bzl_of = |language: &str| {
        manifests
            .iter()
            .find(|manifest| manifest.name == language)
            .and_then(|manifest| manifest.quality.as_ref())
            .map(|quality| quality.bzl.clone())
    };
    for language in unmarked {
        let bzl = bzl_of(language);
        assert!(
            marks
                .iter()
                .any(|entry| !entry.marks.is_empty() && bzl_of(&entry.language) == bzl),
            "{language} shares no quality.bzl with a language that has files"
        );
    }
}

fn quality_languages_by_name() -> usize {
    shipped()
        .iter()
        .filter(|manifest| manifest.quality.is_some())
        .count()
}

#[test]
fn every_command_is_a_rules_dx_label() {
    let all: Vec<&str> = command_labels().collect();
    assert!(all.len() > COMMAND_TARGETS.len(), "{all:?}");
    for label in all {
        assert!(label.starts_with("@rules_dx//"), "{label}");
    }
}

#[test]
fn command_looks_up_a_tool_of_a_language() {
    assert_eq!(command("go", "go"), Some("@rules_dx//go/private/tools:go"));
    assert_eq!(command("go", "mix"), None);
    assert_eq!(command("nope", "go"), None);
    assert_eq!(
        language_commands("elixir"),
        ["@rules_dx//elixir/private/tools:mix"]
    );
    assert_eq!(
        language_commands("rust"),
        [
            "@rules_dx//rust/private/ide:discover",
            "@rules_dx//rust/private/ide:flycheck"
        ]
    );
    assert!(language_commands("haskell").is_empty());
}

#[test]
fn repo_relative_drops_the_rules_dx_repo() {
    assert_eq!(repo_relative(GENERATE), "dx:generate");
    assert_eq!(repo_relative(DX), ":dx");
    assert_eq!(repo_relative(TOOL_RESOLVE), "dx/tool_resolve");
}

#[test]
fn resolvers_name_the_setup_hub_resolver_labels() {
    assert_eq!(resolvers("zig"), ["@rules_dx//zig/private/tools/lock"]);
    assert!(resolvers("go").is_empty());
    assert!(resolvers("kotlin").is_empty());
}
