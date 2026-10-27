#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, serde_json, Run};

pub const CLEAN: &str = "def add(a: int, b: int) -> int:\n    return a + b\n";

pub const DIRTY_FORMAT: &str = "def add(a:int,b:int)->int:\n    return a+b\n";

pub const DIRTY_LINT: &str = "import os\n\n\ndef add(a: int, b: int) -> int:\n    return a + b\n";

pub const MODULE: &str = r#"module(name = "renamed_quality_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_python", version = "1.9.0")
bazel_dep(name = "aspect_rules_py", version = "2.0.0-alpha.6")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")
"#;

pub const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "ruff_config")
load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "python_family",
    family_id = "python",
    disabled = ["audit"],
    format = ["ruff"],
    lint = ["ruff"],
    typecheck = ["ty"],
)

workspace_policy(
    name = "policy",
    families = [":python_family"],
)

ruff_config(
    name = "ruff",
    src = "ruff.toml",
    visibility = ["//visibility:public"],
)
"#;

pub const RUFF_TOML: &str = "line-length = 88\n";

pub fn library(name: &str) -> String {
    format!(
        "python_library(\n    name = \"{name}\",\n    srcs = [\"{name}.py\"],\n    aspect_hints = [\"//:ruff\"],\n)\n"
    )
}

pub fn package(names: &[&str]) -> String {
    let libraries: Vec<String> = names.iter().map(|name| library(name)).collect();
    format!(
        "load(\"@rules_dx//python/rules:defs.bzl\", \"python_library\")\n\n{}",
        libraries.join("\n")
    )
}

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

pub fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("quality/tests/check_continuation_common.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

pub const TYPE_ERROR: &str = "def total() -> int:\n    return \"x\"\n";

pub const DX_BUILD: &str = r#"alias(
    name = "generate",
    actual = "@rules_dx//dx:generate",
    visibility = ["//visibility:public"],
)

alias(
    name = "generate_check",
    actual = "@rules_dx//dx:generate_check",
    visibility = ["//visibility:public"],
)
"#;

pub struct Sources {
    pub app: &'static str,
    pub fmt: &'static str,
    pub lint: &'static str,
    pub typed: &'static str,
    pub drift: bool,
}

pub const CLEAN_SOURCES: Sources = Sources {
    app: CLEAN,
    fmt: CLEAN,
    lint: CLEAN,
    typed: CLEAN,
    drift: false,
};

pub const DRIFT_SOURCES: Sources = Sources {
    app: CLEAN,
    fmt: DIRTY_FORMAT,
    lint: DIRTY_LINT,
    typed: TYPE_ERROR,
    drift: true,
};

pub fn consumer(sources: &Sources) -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("ruff.toml"), RUFF_TOML);
    write(
        &root.join("app/BUILD.bazel"),
        &package(&["app", "fmt", "lint", "typed"]),
    );
    write(&root.join("gen/BUILD.bazel"), &package(&["mod"]));
    write(&root.join("app/app.py"), sources.app);
    write(&root.join("app/fmt.py"), sources.fmt);
    write(&root.join("app/lint.py"), sources.lint);
    write(&root.join("app/typed.py"), sources.typed);
    write(&root.join("gen/mod.py"), CLEAN);
    write(&root.join("dx/BUILD.bazel"), DX_BUILD);
    let generated = dx(&root, "generate", "//...", &["--apply"]);
    assert_eq!(generated.status.code(), Some(0), "{}", generated.combined());
    if sources.drift {
        write(&root.join("gen/extra.py"), CLEAN);
    }
    root
}

pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{dir:?}: {error}"));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("entry in {dir:?}: {error}"))
                .path();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.starts_with("bazel-") || name == ".git" || name == "MODULE.bazel.lock" {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or_else(|error| panic!("{path:?}: {error}"))
                    .display()
                    .to_string();
                out.insert(rel, std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

pub fn assert_same(root: &Path, before: &BTreeMap<String, Vec<u8>>) {
    let after = snapshot(root);
    let changed: Vec<&String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .collect();
    assert!(changed.is_empty(), "check changed {changed:?}");
}

pub fn dx(root: &Case, command: &str, scope: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("check-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        scope,
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    root.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

pub fn events(run: &Run) -> Vec<serde_json::Value> {
    run.stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("json {line:?}: {error}"))
        })
        .collect()
}

pub fn of_kind(events: &[serde_json::Value], kind: &str) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter(|event| event["event"] == kind)
        .cloned()
        .collect()
}

pub fn phase_exits(events: &[serde_json::Value]) -> Vec<(String, i64)> {
    of_kind(events, "phase_finished")
        .iter()
        .map(|event| {
            (
                event["phase"].as_str().unwrap_or("?").to_owned(),
                event["exit_code"].as_i64().unwrap_or(-1),
            )
        })
        .collect()
}

pub fn exits(format: i64, lint: i64, typecheck: i64, generate: i64) -> Vec<(String, i64)> {
    vec![
        ("format".to_owned(), format),
        ("lint".to_owned(), lint),
        ("typecheck".to_owned(), typecheck),
        ("generate".to_owned(), generate),
    ]
}

pub fn finished(events: &[serde_json::Value]) -> serde_json::Value {
    events
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("no events"))
}
