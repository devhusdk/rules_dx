use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case, FakeBazel, Reply};
use dx_test_scratch::{scratch, TempDir};
use dx_testing::{resolve_runfiles, Run};

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

fn workspace() -> Case {
    let root = nested::case("discovery");
    write(&root.join("MODULE.bazel"), &nested::rules_dx_module("", ""));
    write(&root.join(".bazelrc"), BAZELRC);
    nested::write_rules_dx_config(&root);
    write(&root.join("pkg/defs.bzl"), DEFS);
    write(&root.join("pkg/BUILD.bazel"), GOOD_BUILD);
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

const QUERY_UNIT: &str =
    "--nohome_rc --nosystem_rc query --output=label_kind -- tests(//pkg:unit_test)";

const CQUERY_UNIT: &str =
    "--nohome_rc --nosystem_rc cquery --@rules_dx//config:clippy_output_diagnostics=true --@rules_dx//config:rustc_output_diagnostics=true --output=label_kind -- set(\"//pkg:unit_test\")";

const QUERY_ALIAS: &str =
    "--nohome_rc --nosystem_rc query --output=label_kind -- tests(//pkg:unit_alias)";

const QUERY_BAD: &str =
    "--nohome_rc --nosystem_rc query --output=label_kind -- tests(//pkg:bad_test)";

const CQUERY_BAD: &str =
    "--nohome_rc --nosystem_rc cquery --@rules_dx//config:clippy_output_diagnostics=true --@rules_dx//config:rustc_output_diagnostics=true --output=label_kind -- set(\"//pkg:bad_test\")";

const ANALYSIS_ERROR: &str = "ERROR: pkg/BUILD.bazel:3:9: in bad_test rule //pkg:bad_test: \nError in fail: analysis boom\nERROR: Analysis of target '//pkg:bad_test' failed\n";

struct Recorded {
    fake: FakeBazel,
    workspace: PathBuf,
    _dir: TempDir,
}

fn recorded(replies: &[Reply]) -> Recorded {
    let dir = scratch("configured-discovery");
    let workspace = dir.path().join("ws");
    write(
        &workspace.join("MODULE.bazel"),
        &nested::rules_dx_module("", ""),
    );
    Recorded {
        fake: FakeBazel::new(&dir.path().join("fake"), replies),
        workspace,
        _dir: dir,
    }
}

fn dx_recorded(recorded: &Recorded, args: &[&str]) -> Run {
    let workspace = recorded.workspace.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    dx_testing::run(&dx(), &argv, &[("PATH", recorded.fake.path().as_str())])
        .unwrap_or_else(|error| panic!("run dx: {error}"))
}

#[test]
fn real_bazel_lists_configured_tests_and_runs_the_file_owner_tests() {
    let root = workspace();
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

    let result = dx_in(&root, &["test", "pkg/source.txt"]);
    assert_eq!(result.status.code(), Some(0), "{}", result.combined());
    let text = result.combined();
    assert!(text.contains("//pkg:unit_test"), "{text}");
    assert!(text.contains("PASSED"), "{text}");
    assert!(!text.contains("//pkg:guarded_test"), "{text}");
}

#[test]
fn configured_listing_names_the_cquery_verb_without_tests_function() {
    let recorded = recorded(&[
        Reply {
            args: QUERY_UNIT,
            stdout: "custom_test rule //pkg:unit_test\n",
            stderr: "Loading: 0 packages loaded\n",
            exit: 0,
        },
        Reply {
            args: CQUERY_UNIT,
            stdout: "custom_test rule //pkg:unit_test (e722a21)\n",
            stderr: "INFO: Found 1 target...\n",
            exit: 0,
        },
    ]);
    let json = dx_recorded(
        &recorded,
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
    assert_eq!(recorded.fake.calls(), vec![QUERY_UNIT, CQUERY_UNIT]);
}

#[test]
fn alias_scope_is_not_reported_as_a_test() {
    let recorded = recorded(&[Reply {
        args: QUERY_ALIAS,
        stdout: "",
        stderr: "INFO: Empty results\n",
        exit: 0,
    }]);
    let alias = dx_recorded(
        &recorded,
        &["list-tests", "--configured", "//pkg:unit_alias"],
    );
    assert_eq!(alias.status.code(), Some(1), "{}", alias.combined());
    assert!(alias.stdout.trim().is_empty(), "{}", alias.stdout);
    assert!(alias.stderr.contains("no_tests"), "{}", alias.stderr);
}

#[test]
fn analysis_failure_is_reported_not_hidden() {
    let recorded = recorded(&[
        Reply {
            args: QUERY_BAD,
            stdout: "bad_test rule //pkg:bad_test\n",
            stderr: "",
            exit: 0,
        },
        Reply {
            args: CQUERY_BAD,
            stdout: "",
            stderr: ANALYSIS_ERROR,
            exit: 1,
        },
    ]);
    let result = dx_recorded(&recorded, &["list-tests", "--configured", "//pkg:bad_test"]);
    assert_eq!(result.status.code(), Some(1), "{}", result.combined());
    assert!(result.stdout.trim().is_empty(), "{}", result.stdout);
    assert!(result.stderr.contains("bazel_failed"), "{}", result.stderr);
}

#[test]
fn retired_plural_spelling_is_rejected_without_running_bazel() {
    let recorded = recorded(&[]);
    let result = dx_recorded(&recorded, &["tests", "--configured", "//pkg/..."]);
    assert_eq!(result.status.code(), Some(2), "{}", result.combined());
    assert!(
        result.stderr.contains("unsupported command 'tests'"),
        "{}",
        result.stderr
    );
    assert!(recorded.fake.calls().is_empty(), "bazel was started");
}
