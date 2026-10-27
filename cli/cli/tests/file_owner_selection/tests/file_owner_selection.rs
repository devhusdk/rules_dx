use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case, FakeBazel, Reply};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const MODULE: &str = "module(name = \"fixture\", repo_name = \"rules_dx\")\n";

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

def _flag_impl(_ctx):
    return []

bool_flag = rule(implementation = _flag_impl, build_setting = config.bool(flag = True))
"##;

const CONFIG_BUILD: &str = r#"load("//pkg:defs.bzl", "bool_flag")

bool_flag(name = "validate", build_setting_default = True)

bool_flag(name = "clippy_output_diagnostics", build_setting_default = False)

bool_flag(name = "rustc_output_diagnostics", build_setting_default = False)
"#;

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
    write(&root.join("MODULE.bazel"), MODULE);
    write(&root.join(".bazelrc"), BAZELRC);
    write(&root.join("config/BUILD.bazel"), CONFIG_BUILD);
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

struct Recorded {
    fake: FakeBazel,
    workspace: PathBuf,
}

fn recorded(replies: &[Reply]) -> Recorded {
    let dir = mkscratch("file-owner-selection").unwrap_or_else(|error| panic!("scratch: {error}"));
    let workspace = dir.join("ws");
    write(&workspace.join("MODULE.bazel"), MODULE);
    Recorded {
        fake: FakeBazel::new(&dir.join("fake"), replies),
        workspace,
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
fn real_bazel_runs_the_file_owner_tests() {
    let root = workspace();
    let result = dx_in(&root, &["test", "pkg/source.txt"]);
    assert_eq!(result.status.code(), Some(0), "{}", result.combined());
    let text = result.combined();
    assert!(text.contains("//pkg:unit_test"), "{text}");
    assert!(text.contains("PASSED"), "{text}");
    assert!(!text.contains("//pkg:guarded_test"), "{text}");
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

#[test]
fn list_tests_is_rejected_without_running_bazel() {
    let recorded = recorded(&[]);
    let result = dx_recorded(&recorded, &["list-tests", "//..."]);
    assert_eq!(result.status.code(), Some(2), "{}", result.combined());
    assert!(
        result.stderr.contains("unsupported command 'list-tests'"),
        "{}",
        result.stderr
    );
    assert!(result.stderr.contains("bazel query"), "{}", result.stderr);
    assert!(recorded.fake.calls().is_empty(), "bazel was started");
}
