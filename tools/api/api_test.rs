mod page;
mod snapshot;

#[test]
fn checked_in_snapshot_matches_the_code() {
    let builds = |paths: Vec<String>| -> Vec<(String, String)> {
        paths
            .into_iter()
            .map(|path| {
                let package = snapshot::package_of(&path).expect("a package BUILD file");
                (
                    package,
                    dx_testing::read_text(&dx_testing::resolve_runfiles(&path)),
                )
            })
            .collect()
    };
    let builds = snapshot::Builds {
        langs: builds(snapshot::lang_build_paths().expect("DX_API_LANG_BUILDS is set")),
        commands: builds(snapshot::command_build_paths().expect("DX_API_COMMAND_BUILDS is set")),
        packages: builds(snapshot::package_build_paths().expect("DX_API_PACKAGE_BUILDS is set")),
    };
    let starlark: Vec<Vec<u8>> = dx_testing::env_files("DX_API_STARLARK")
        .iter()
        .map(|path| {
            std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        })
        .collect();
    let forwards: Vec<String> = dx_testing::env_files("DX_API_FORWARDS")
        .iter()
        .map(|path| dx_testing::read_text(path))
        .collect();
    let files = snapshot::render(&builds, &starlark, &forwards).expect("snapshot renders");
    let paths = dx_testing::env_text("DX_API_SNAPSHOT");
    let mut stale = Vec::new();
    for (name, text) in &files {
        let key = paths
            .split_whitespace()
            .find(|key| key.ends_with(&format!("api/{name}")))
            .unwrap_or_else(|| panic!("api/{name} is not in the test data"));
        let path = dx_testing::rlocation(key).unwrap_or_else(|error| panic!("{key}: {error}"));
        let checked_in = dx_testing::read_text(&path);
        if *name == "schemas.txt" {
            let problems = snapshot::unbumped_schemas(&checked_in, text);
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        }
        if checked_in != *text {
            stale.push(format!("api/{name}"));
        }
    }
    let surface = page::render(
        &files["cli.json"],
        &files["starlark.json"],
        &files["labels.txt"],
        &files["schemas.txt"],
    )
    .expect("page renders");
    let path = dx_testing::rlocation(&dx_testing::env_text("DX_SURFACE_PAGE"))
        .unwrap_or_else(|error| panic!("{}: {error}", page::PATH));
    if dx_testing::read_text(&path) != surface {
        stale.push(page::PATH.to_owned());
    }
    assert!(
        stale.is_empty(),
        "the public API changed: {}. If the change is intended, run `bazel run //tools/api:update` and commit the result",
        stale.join(", ")
    );
}

fn extraction_gaps(public: &[&str], extracted: &[&str], pending: &[&str]) -> Vec<String> {
    let public: Vec<String> = public
        .iter()
        .map(|path| match path.rsplit_once('/') {
            Some((package, file)) => format!("//{package}:{file}"),
            None => format!("//:{path}"),
        })
        .collect();
    let mut gaps = Vec::new();
    for label in &public {
        if !extracted.contains(&label.as_str()) && !pending.contains(&label.as_str()) {
            gaps.push(format!(
                "{label} is public but not extracted: add it to `_STARLARK_SRCS` in tools/api/BUILD.bazel"
            ));
        }
    }
    for label in pending {
        if extracted.contains(label) {
            gaps.push(format!(
                "{label} is extracted: drop it from `_NOT_EXTRACTED` in tools/api/BUILD.bazel"
            ));
        } else if !public.iter().any(|public| public == label) {
            gaps.push(format!(
                "{label} is not a public file: drop it from `_NOT_EXTRACTED` in tools/api/BUILD.bazel"
            ));
        }
    }
    gaps
}

fn canonical_defaults(snapshot: &serde_json::Value) -> Vec<String> {
    fn walk(value: &serde_json::Value, load: &str, hits: &mut Vec<String>) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, load, hits);
                }
            }
            serde_json::Value::Object(object) => {
                let load = object
                    .get("load")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(load);
                if let Some(default) = object.get("default").and_then(serde_json::Value::as_str) {
                    if default.contains("@@") {
                        let name = object
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default();
                        hits.push(format!("{load} {name}"));
                    }
                }
                for item in object.values() {
                    walk(item, load, hits);
                }
            }
            _ => {}
        }
    }
    let mut hits = Vec::new();
    walk(snapshot, "", &mut hits);
    hits
}

#[test]
fn no_public_default_names_a_canonical_repo() {
    let paths = dx_testing::env_text("DX_API_SNAPSHOT");
    let key = paths
        .split_whitespace()
        .find(|key| key.ends_with("api/starlark.json"))
        .expect("api/starlark.json is in the test data");
    let path = dx_testing::rlocation(key).unwrap_or_else(|error| panic!("{key}: {error}"));
    let snapshot: serde_json::Value =
        serde_json::from_str(&dx_testing::read_text(&path)).expect("api/starlark.json parses");
    let hits = canonical_defaults(&snapshot);
    assert!(
        hits.is_empty(),
        "these defaults name a canonical @@ repository, which differs in each consumer; default to None and resolve it in the body:\n{}",
        hits.join("\n")
    );
}

#[test]
fn canonical_default_is_a_hit_and_none_is_not() {
    let snapshot = serde_json::json!([{
        "load": "@rules_dx//qt:defs.bzl",
        "macros": [{
            "name": "qt_cc_library",
            "params": [
                {"name": "qt", "default": "\"@@+qt+dx_qt\""},
                {"name": "deps", "default": "None"},
            ],
        }],
    }]);
    assert_eq!(
        canonical_defaults(&snapshot),
        vec!["@rules_dx//qt:defs.bzl qt".to_owned()]
    );
}

#[test]
fn every_public_bzl_file_is_extracted() {
    let public = dx_testing::env_text("DX_API_PUBLIC_BZL");
    let extracted = dx_testing::env_text("DX_API_EXTRACTED");
    let pending = std::env::var("DX_API_NOT_EXTRACTED").unwrap_or_default();
    let public: Vec<&str> = public.split_whitespace().collect();
    assert!(!public.is_empty(), "DX_API_PUBLIC_BZL lists no files");
    let extracted: Vec<&str> = extracted.split_whitespace().collect();
    let pending: Vec<&str> = pending.split_whitespace().collect();
    let gaps = extraction_gaps(&public, &extracted, &pending);
    assert!(gaps.is_empty(), "{}", gaps.join("\n"));
}

#[test]
fn public_file_in_neither_list_is_a_gap() {
    let gaps = extraction_gaps(&["go/defs.bzl"], &[], &[]);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].starts_with("//go:defs.bzl is public but not extracted"),
        "{gaps:?}"
    );
}

#[test]
fn pending_file_that_is_extracted_is_a_gap() {
    let gaps = extraction_gaps(&["go/defs.bzl"], &["//go:defs.bzl"], &["//go:defs.bzl"]);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].starts_with("//go:defs.bzl is extracted"),
        "{gaps:?}"
    );
}

#[test]
fn pending_file_that_is_not_public_is_a_gap() {
    let gaps = extraction_gaps(&["go/defs.bzl"], &["//go:defs.bzl"], &["//gone:defs.bzl"]);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps[0].starts_with("//gone:defs.bzl is not a public file"),
        "{gaps:?}"
    );
}
