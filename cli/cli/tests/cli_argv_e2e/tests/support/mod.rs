use std::path::{Path, PathBuf};

use dx_nested::{rules_dx_module, write_rules_dx_config};
use dx_testing::{resolve_runfiles, run, Run};

const BAZELRC: &str = "common:dx_dev --define=dx=dev\ncommon:dx_release --define=dx=release\n";

/// Test rule whose launcher is a shell script. It records its argv as JSON and
/// exits 1 when the sentinel argument is missing. The `ignored` flavour mimics
/// an `#[ignore]` libtest case: it only writes its marker when `--ignored` is
/// among the arguments. Both print what they wrote, so the e2e can read it
/// from the test log (`--test_output=all`).
const DEFS: &str = r##"def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, """#!/bin/sh
dir="$TEST_UNDECLARED_OUTPUTS_DIR"
json="["
sentinel=0
ignored=0
sep=""
for arg in "$@"; do
  json="$json$sep\\"$arg\\""
  sep=","
  [ "$arg" = "--sentinel" ] && sentinel=1
  [ "$arg" = "--ignored" ] && ignored=1
done
json="$json]"
printf '%s' "$json" > "$dir/argv.json"
printf 'ARGV_JSON:%s\\n' "$json"
if [ "{ignore_case}" = "1" ]; then
  if [ "$ignored" = "1" ]; then
    : > "$dir/marker"
    echo MARKER_WRITTEN
  fi
  exit 0
fi
if [ "$sentinel" = "0" ]; then
  echo "sentinel missing" >&2
  exit 1
fi
""".format(ignore_case = "1" if ctx.attr.ignore_case else "0"), is_executable = True)
    return [DefaultInfo(executable = out)]

argv_test = rule(
    implementation = _impl,
    test = True,
    attrs = {"ignore_case": attr.bool()},
)
"##;

const BUILD: &str = r#"load("//:defs.bzl", "argv_test")

argv_test(name = "argv_test")

argv_test(name = "ignored_test", ignore_case = True)
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

pub fn fixture(root: &Path) {
    write(&root.join("MODULE.bazel"), &rules_dx_module("", ""));
    write(&root.join(".bazelrc"), BAZELRC);
    write_rules_dx_config(root);
    write(&root.join("BUILD.bazel"), "");
    write(&root.join(".gitignore"), "/bazel-*\n/MODULE.bazel.lock\n");
    write(&root.join("defs.bzl"), DEFS);
    write(&root.join("t/BUILD.bazel"), BUILD);
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
