use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const DECLARED: &[&str] = &[
    "apple/bundle/defs.bzl",
    "apple/provisioning/defs.bzl",
    "config/BUILD.bazel",
    "deploy/rules/archive.bzl",
    "erlang/rules/defs.bzl",
    "erlang/toolchain/repos.bzl",
    "libs/starlark/wrapper.bzl",
    "qt/rules/defs.bzl",
    "qt/sdk/launcher.c",
    "quality/pipeline.bzl",
];

/// The `.bzl` files that nested consumers of each area load first.
const ENTRY_POINTS: &[&str] = &[
    "//apple/bundle:defs.bzl",
    "//apple/provisioning:defs.bzl",
    "//deploy/rules:archive.bzl",
    "//deploy/rules:defs.bzl",
    "//erlang/rules:defs.bzl",
    "//erlang/toolchain:repos.bzl",
    "//qt/rules:defs.bzl",
    "//qt/sdk:repos.bzl",
];

#[test]
fn area_module_srcs_carry_the_sources_nested_consumers_read() {
    for rel in DECLARED {
        let path = dx_testing::resolve_runfiles(rel);
        assert!(path.is_file(), "{rel} is not a file at {path:?}");
    }
}

#[test]
fn rules_dx_source_names_the_checkout() {
    let source = dx_testing::rules_dx_source();
    assert!(source.join("MODULE.bazel").is_file(), "{source:?}");
}

/// A loaded `.bzl` file named by its package and its path from the module root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Bzl {
    package: String,
    path: String,
}

/// Resolves one `load` label against the package of the loading file; `None` leaves the module.
fn resolve(label: &str, package: &str) -> Option<Bzl> {
    let local = if let Some(rest) = label.strip_prefix("@rules_dx//") {
        format!("//{rest}")
    } else if let Some(rest) = label.strip_prefix("@//") {
        format!("//{rest}")
    } else if label.starts_with('@') {
        return None;
    } else {
        label.to_owned()
    };
    let (package, name) = match local.strip_prefix("//") {
        Some(rest) => {
            let (package, name) = rest
                .split_once(':')
                .unwrap_or_else(|| panic!("load label {label} names no file"));
            (package.to_owned(), name.to_owned())
        }
        None => (
            package.to_owned(),
            local.strip_prefix(':').unwrap_or(&local).to_owned(),
        ),
    };
    let path = if package.is_empty() {
        name
    } else {
        format!("{package}/{name}")
    };
    Some(Bzl { package, path })
}

/// Returns the first string argument of every `load(` call in `text`, single- or multi-line.
fn loads(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (start, _) in text.match_indices("load(") {
        let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let prefix = &text[line_start..start];
        if !prefix.trim().is_empty() {
            continue;
        }
        let rest = text[start + "load(".len()..].trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let body = &rest[1..];
        if let Some(end) = body.find(quote) {
            found.push(body[..end].to_owned());
        }
    }
    found
}

fn in_runfiles(rel: &str) -> Option<PathBuf> {
    dx_testing::rlocation(&dx_testing::rlocation_key(rel))
        .ok()
        .filter(|path| path.is_file())
}

#[test]
fn load_parser_follows_multi_line_and_repository_forms() {
    let text = "load(\n    \"@rules_dx//erlang/toolchain:repos.bzl\",\n    \"x\",\n)\n# load(\"//no:pe.bzl\")\nload(\":sibling.bzl\", \"y\")\nload(\"@bazel_skylib//lib:paths.bzl\", \"paths\")\n";
    let labels = loads(text);
    assert_eq!(
        labels,
        [
            "@rules_dx//erlang/toolchain:repos.bzl",
            ":sibling.bzl",
            "@bazel_skylib//lib:paths.bzl"
        ]
    );
    let resolved: Vec<Option<Bzl>> = labels.iter().map(|l| resolve(l, "quality")).collect();
    assert_eq!(
        resolved,
        [
            Some(Bzl {
                package: "erlang/toolchain".into(),
                path: "erlang/toolchain/repos.bzl".into()
            }),
            Some(Bzl {
                package: "quality".into(),
                path: "quality/sibling.bzl".into()
            }),
            None,
        ]
    );
}

#[test]
fn area_module_srcs_carry_the_load_closure_of_their_entry_points() {
    let mut queue: Vec<(Bzl, String)> = ENTRY_POINTS
        .iter()
        .map(|label| {
            (
                resolve(label, "").expect("entry points stay in rules_dx"),
                "ENTRY_POINTS".to_owned(),
            )
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut missing = BTreeMap::new();
    while let Some((bzl, loader)) = queue.pop() {
        if !seen.insert(bzl.clone()) {
            continue;
        }
        let build = ["BUILD.bazel", "BUILD"]
            .iter()
            .map(|name| {
                if bzl.package.is_empty() {
                    (*name).to_owned()
                } else {
                    format!("{}/{name}", bzl.package)
                }
            })
            .find(|rel| in_runfiles(rel).is_some());
        if build.is_none() {
            missing
                .entry(format!("{}/BUILD.bazel", bzl.package))
                .or_insert_with(|| format!("the package of {}", bzl.path));
        }
        let Some(path) = in_runfiles(&bzl.path) else {
            missing.entry(bzl.path.clone()).or_insert(loader);
            continue;
        };
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        for label in loads(&text) {
            if let Some(next) = resolve(&label, &bzl.package) {
                queue.push((next, bzl.path.clone()));
            }
        }
    }
    let report: Vec<String> = missing
        .iter()
        .map(|(file, loader)| format!("{file} (needed by {loader})"))
        .collect();
    assert!(
        report.is_empty(),
        "nested consumers load files no area module_srcs carries; add their package to //:module_srcs or the area aggregate:\n{}",
        report.join("\n")
    );
}
