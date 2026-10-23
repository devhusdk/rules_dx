use std::path::{Path, PathBuf};
use std::process::Command;

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

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

const A_BUILD: &str = r#"load("//:defs.bzl", "custom_test")

filegroup(name = "lib", srcs = ["a.txt"], visibility = ["//visibility:public"])

custom_test(name = "a_test", data = [":lib"])
"#;

const B_BUILD: &str = r#"load("//:defs.bzl", "custom_test")

filegroup(name = "lib", srcs = ["b.txt", "//a:lib"], visibility = ["//visibility:public"])

custom_test(name = "b_test", data = [":lib"])
"#;

const C_BUILD: &str = r#"load("//:defs.bzl", "custom_test")

custom_test(name = "c_test", data = ["c.txt"])
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

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=dx",
            "-c",
            "user.email=dx@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap_or_else(|error| panic!("git: {error}"));
    assert!(status.success(), "git {args:?}");
}

fn repo(name: &str) -> PathBuf {
    let root = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    write(&root.join("MODULE.bazel"), "module(name = \"fixture\")\n");
    write(&root.join("BUILD.bazel"), "");
    write(&root.join(".gitignore"), "/bazel-*\n");
    write(&root.join("defs.bzl"), DEFS);
    write(&root.join("a/BUILD.bazel"), A_BUILD);
    write(&root.join("a/a.txt"), "a\n");
    write(&root.join("b/BUILD.bazel"), B_BUILD);
    write(&root.join("b/b.txt"), "b\n");
    write(&root.join("c/BUILD.bazel"), C_BUILD);
    write(&root.join("c/c.txt"), "c\n");
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    root
}

fn dx_in(root: &Path, args: &[&str]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    run(&dx(), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}

fn lines(run: &Run) -> Vec<String> {
    run.stdout.lines().map(str::to_owned).collect()
}

#[test]
fn test_affected_runs_changed_lib_tests_and_reverse_deps_only() {
    let root = repo("dx-affected-lib");
    write(&root.join("a/a.txt"), "changed\n");

    let listed = dx_in(&root, &["list-tests", "--affected"]);
    assert_eq!(listed.status.code(), Some(0), "{}", listed.combined());
    assert_eq!(lines(&listed), vec!["//a:a_test", "//b:b_test"]);

    let ran = dx_in(&root, &["test", "--affected"]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let all = ran.combined();
    assert!(all.contains("//a:a_test"), "{all}");
    assert!(all.contains("//b:b_test"), "{all}");
    assert!(!all.contains("//c:c_test"), "{all}");
}

#[test]
fn test_affected_json_reports_the_affected_object() {
    let root = repo("dx-affected-json");
    write(&root.join("b/b.txt"), "changed\n");
    let json = dx_in(&root, &["list-tests", "--affected", "--output=json"]);
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    let first = json.stdout.lines().next().unwrap_or_default();
    assert!(first.contains("\"command_started\""), "{first}");
    assert!(first.contains("\"changed_files\":[\"b/b.txt\"]"), "{first}");
    assert!(first.contains("\"widened\":[]"), "{first}");
    assert!(first.contains("//b:b_test"), "{first}");
    assert!(!first.contains("//a:a_test"), "{first}");
}

#[test]
fn untracked_files_count_and_a_clean_tree_selects_nothing() {
    let root = repo("dx-affected-clean");
    let clean = dx_in(&root, &["test", "--affected"]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(clean.stdout.trim(), "no affected targets");

    write(&root.join("c/c.txt"), "changed\n");
    let dirty = dx_in(&root, &["list-tests", "--affected"]);
    assert_eq!(lines(&dirty), vec!["//c:c_test"]);
}

#[test]
fn module_change_widens_to_the_whole_workspace() {
    let root = repo("dx-affected-widen");
    write(
        &root.join("MODULE.bazel"),
        "module(name = \"fixture\", version = \"1\")\n",
    );
    let listed = dx_in(&root, &["list-tests", "--affected"]);
    assert_eq!(listed.status.code(), Some(0), "{}", listed.combined());
    assert!(
        listed.stderr.contains("widened to //..."),
        "{}",
        listed.combined()
    );
    assert_eq!(lines(&listed).len(), 3, "{}", listed.combined());
}

#[test]
fn affected_conflicts_with_scopes_and_here() {
    let root = repo("dx-affected-usage");
    let scoped = dx_in(&root, &["test", "--affected", "//a:a_test"]);
    assert_eq!(scoped.status.code(), Some(2), "{}", scoped.combined());
    let here = dx_in(&root, &["test", "--affected", "--here"]);
    assert_eq!(here.status.code(), Some(2), "{}", here.combined());
}
