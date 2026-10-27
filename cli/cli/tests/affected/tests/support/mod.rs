use std::path::{Path, PathBuf};
use std::process::Command;

pub use dx_nested::rules_dx_module;
use dx_nested::write_rules_dx_config;
use dx_testing::{resolve_runfiles, run, Run};

const BAZELRC: &str = "common:dx_dev --define=dx=dev\ncommon:dx_release --define=dx=release\n";

const DEFS: &str = r##"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + "@SUFFIX@")
    ctx.actions.write(out, "@BODY@", is_executable = True)
    return [DefaultInfo(executable = out, runfiles = ctx.runfiles(files = ctx.files.data))]

custom_test = rule(
    implementation = _impl,
    test = True,
    attrs = {"data": attr.label_list(allow_files = True)},
)
"##;

const TEST_LAUNCHER: (&str, &str) = if cfg!(windows) {
    (".bat", r"@exit /b 0\r\n")
} else {
    ("", r"#!/bin/sh\nexit 0\n")
};

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

pub fn write(path: &Path, text: &str) {
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

pub fn commit(root: &Path) {
    git(root, &["init", "--quiet", "--initial-branch=main"]);
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "base"]);
}

pub fn fixture(root: &Path) {
    write(&root.join("MODULE.bazel"), &rules_dx_module("", ""));
    write(&root.join(".bazelrc"), BAZELRC);
    write_rules_dx_config(root);
    write(&root.join("BUILD.bazel"), "");
    write(&root.join(".gitignore"), "/bazel-*\n/MODULE.bazel.lock\n");
    let (suffix, body) = TEST_LAUNCHER;
    write(
        &root.join("defs.bzl"),
        &DEFS.replace("@SUFFIX@", suffix).replace("@BODY@", body),
    );
    write(&root.join("a/BUILD.bazel"), A_BUILD);
    write(&root.join("a/a.txt"), "a\n");
    write(&root.join("b/BUILD.bazel"), B_BUILD);
    write(&root.join("b/b.txt"), "b\n");
    write(&root.join("c/BUILD.bazel"), C_BUILD);
    write(&root.join("c/c.txt"), "c\n");
}

pub fn dx_in(root: &Path, args: &[&str]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    let at = argv
        .iter()
        .position(|arg| *arg == "--")
        .unwrap_or(argv.len());
    argv.splice(at..at, ["--workspace", workspace.as_str()]);
    run(&dx(), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}
