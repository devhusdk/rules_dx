use std::path::{Path, PathBuf};

use dx_testing::nested::{self, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = "module(name = \"fixture\")\n";

const BAZELRC: &str = "common:dx_dev --define=dx=dev\ncommon:dx_release --define=dx=release\n";

const DEFS: &str = r##"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, "#!/bin/sh\nexit 0\n", is_executable = True)
    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.files.data))]

custom_test = rule(
    implementation = _impl,
    test = True,
    attrs = {"data": attr.label_list(allow_files = True)},
)

def _bad(ctx):
    fail("analysis boom")

bad_test = rule(implementation = _bad, test = True)
"##;

const GOOD_BUILD: &str = r#"load(":defs.bzl", "custom_test")

config_setting(name = "on", values = {"define": "on=1"})

filegroup(name = "lib", srcs = ["source.txt"])

custom_test(name = "unit_test", data = [":lib"])

custom_test(
    name = "select_test",
    data = select({":on": [":lib"], "//conditions:default": []}),
)

custom_test(
    name = "guarded_test",
    data = select({":on": [":missing_nowhere"], "//conditions:default": []}),
)

alias(name = "unit_alias", actual = ":unit_test", testonly = True)

test_suite(name = "suite", tests = [":unit_test", ":select_test"])
"#;

const BAD_BUILD: &str = "load(\":defs.bzl\", \"bad_test\")\n\nbad_test(name = \"bad_test\")\n";

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn workspace(build: &str) -> Case {
    let root = nested::case("discovery");
    write(&root.join("MODULE.bazel"), MODULE);
    write(&root.join(".bazelrc"), BAZELRC);
    write(&root.join("pkg/defs.bzl"), DEFS);
    write(&root.join("pkg/BUILD.bazel"), build);
    write(&root.join("pkg/source.txt"), "");
    root
}

fn dx_in(root: &Case, args: &[&str]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    root.run(&dx(), &argv, &[])
}

fn lines(run: &Run) -> Vec<String> {
    run.stdout.lines().map(str::to_owned).collect()
}

fn assert_plain_labels(labels: &[String]) {
    for label in labels {
        assert!(label.starts_with("//pkg:"), "label {label}");
        assert!(
            !label.contains(' ') && !label.contains('('),
            "label {label}"
        );
    }
}

#[test]
fn configured_listing_covers_pattern_suite_and_custom_rules() {
    let root = workspace(GOOD_BUILD);
    let all = dx_in(&root, &["list-tests", "--configured", "//pkg/..."]);
    assert_eq!(all.status.code(), Some(0), "{}", all.combined());
    let labels = lines(&all);
    assert_eq!(
        labels,
        vec!["//pkg:guarded_test", "//pkg:select_test", "//pkg:unit_test"]
    );
    assert_plain_labels(&labels);

    let suite = dx_in(&root, &["list-tests", "--configured", "//pkg:suite"]);
    assert_eq!(suite.status.code(), Some(0), "{}", suite.combined());
    assert_eq!(lines(&suite), vec!["//pkg:select_test", "//pkg:unit_test"]);

    let plain = dx_in(&root, &["list-tests", "//pkg/..."]);
    assert_eq!(plain.status.code(), Some(0), "{}", plain.combined());
    assert_eq!(lines(&plain), labels);
}

#[test]
fn configured_listing_names_the_cquery_verb_without_tests_function() {
    let root = workspace(GOOD_BUILD);
    let json = dx_in(
        &root,
        &[
            "list-tests",
            "--configured",
            "--output=json",
            "//pkg:unit_test",
        ],
    );
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    assert!(
        json.stdout.contains("//pkg:unit_test via cquery"),
        "{}",
        json.stdout
    );
    assert!(
        !json.combined().contains("tests() is not"),
        "{}",
        json.combined()
    );
}

#[test]
fn alias_scope_is_not_reported_as_a_test() {
    let root = workspace(GOOD_BUILD);
    let alias = dx_in(&root, &["list-tests", "--configured", "//pkg:unit_alias"]);
    assert_eq!(alias.status.code(), Some(1), "{}", alias.combined());
    assert!(alias.stdout.trim().is_empty(), "{}", alias.stdout);
    assert!(alias.stderr.contains("no_tests"), "{}", alias.stderr);
}

#[test]
fn file_scoped_test_selects_the_runnable_owner_tests() {
    let root = workspace(GOOD_BUILD);
    let result = dx_in(&root, &["test", "pkg/source.txt"]);
    assert_eq!(result.status.code(), Some(0), "{}", result.combined());
    let text = result.combined();
    assert!(text.contains("//pkg:unit_test"), "{text}");
    assert!(text.contains("PASSED"), "{text}");
    assert!(!text.contains("//pkg:guarded_test"), "{text}");
}

#[test]
fn analysis_failure_is_reported_not_hidden() {
    let root = workspace(BAD_BUILD);
    let result = dx_in(&root, &["list-tests", "--configured", "//pkg:bad_test"]);
    assert_eq!(result.status.code(), Some(1), "{}", result.combined());
    assert!(result.stdout.trim().is_empty(), "{}", result.stdout);
    assert!(result.stderr.contains("bazel_failed"), "{}", result.stderr);
}

#[test]
fn retired_plural_spelling_is_rejected_without_running_bazel() {
    let root = nested::case("discovery");
    let result = dx_in(&root, &["tests", "--configured", "//pkg/..."]);
    assert_eq!(result.status.code(), Some(2), "{}", result.combined());
    assert!(
        result.stderr.contains("unsupported command 'tests'"),
        "{}",
        result.stderr
    );
    assert!(
        !result.combined().contains("Invocation ID"),
        "{}",
        result.combined()
    );
}
