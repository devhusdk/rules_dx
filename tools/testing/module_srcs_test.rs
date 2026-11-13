const DECLARED: &[&str] = &[
    "config/BUILD.bazel",
    "deploy/private/archive.bzl",
    "erlang/private/defs.bzl",
    "erlang/private/toolchain/extensions.bzl",
    "core/wrapper.bzl",
    "qt/private/defs.bzl",
    "qt/private/toolchain/launcher.c",
    "core/quality/pipeline.bzl",
];

/// The `.bzl` files that nested consumers of each area load first.
const ENTRY_POINTS: &[&str] = &[
    "//android:defs.bzl",
    "//android:extensions.bzl",
    "//apple:defs.bzl",
    "//cuda:defs.bzl",
    "//cuda:extensions.bzl",
    "//deploy/private:archive.bzl",
    "//deploy/private:defs.bzl",
    "//deploy/private:deployment.bzl",
    "//erlang:defs.bzl",
    "//erlang:extensions.bzl",
    "//qt:defs.bzl",
    "//qt:extensions.bzl",
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

#[path = "module_srcs_closure.rs"]
mod closure;
use closure::{loads, missing_from_runfiles, resolve, Bzl};

#[test]
fn load_parser_follows_multi_line_and_repository_forms() {
    let text = "load(\n    \"@rules_dx//erlang:extensions.bzl\",\n    \"x\",\n)\n# load(\"//no:pe.bzl\")\nload(\":sibling.bzl\", \"y\")\nload(\"@bazel_skylib//lib:paths.bzl\", \"paths\")\n";
    let labels = loads(text);
    assert_eq!(
        labels,
        [
            "@rules_dx//erlang:extensions.bzl",
            ":sibling.bzl",
            "@bazel_skylib//lib:paths.bzl"
        ]
    );
    let resolved: Vec<Option<Bzl>> = labels.iter().map(|l| resolve(l, "core/quality")).collect();
    assert_eq!(
        resolved,
        [
            Some(Bzl {
                package: "erlang".into(),
                path: "erlang/extensions.bzl".into()
            }),
            Some(Bzl {
                package: "core/quality".into(),
                path: "core/quality/sibling.bzl".into()
            }),
            None,
        ]
    );
}

#[test]
fn area_module_srcs_carry_the_load_closure_of_their_entry_points() {
    let entries = ENTRY_POINTS
        .iter()
        .map(|label| {
            (
                resolve(label, "").expect("entry points stay in rules_dx"),
                "ENTRY_POINTS".to_owned(),
            )
        })
        .collect();
    let report = missing_from_runfiles(entries);
    assert!(
        report.is_empty(),
        "nested consumers load files no area module_srcs carries; add their package to //:module_srcs or the area aggregate:\n{}",
        report.join("\n")
    );
}
