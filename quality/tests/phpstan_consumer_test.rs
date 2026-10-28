use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const CLEAN: &str = "<?php\ndeclare(strict_types=1);\n\nnamespace Dx\\Math;\n\nfinal class Calc\n{\n    public function add(int $a, int $b): int\n    {\n        return $a + $b;\n    }\n}\n";
const BAD_RETURN: &str = "<?php\ndeclare(strict_types=1);\n\nnamespace Dx\\Math;\n\nfinal class Calc\n{\n    public function add(int $a, int $b): string\n    {\n        return $a + $b;\n    }\n}\n";
const BASE: &str = "<?php\ndeclare(strict_types=1);\n\nnamespace Dx\\Base;\n\nfinal class Doubler\n{\n    public function twice(int $x): int\n    {\n        return $x * 2;\n    }\n}\n";
const CALLER: &str = "<?php\ndeclare(strict_types=1);\n\nnamespace Dx\\Math;\n\nuse Dx\\Base\\Doubler;\n\nfinal class Calc\n{\n    public function quad(int $x): int\n    {\n        $d = new Doubler();\n        return $d->twice($d->twice($x));\n    }\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "php_family",
    family_id = "php",
    disabled = [
        "audit",
        "format",
        "lint",
    ],
    typecheck = ["phpstan"],
)

workspace_policy(
    name = "policy",
    families = [":php_family"],
)
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//php/rules:defs.bzl", "php_library")

php_library(
    name = "math",
    srcs = ["src/Calc.php"],
)
"#;

const CALLER_BUILD: &str = r#"load("@rules_dx//php/rules:defs.bzl", "php_library")

php_library(
    name = "math",
    srcs = ["src/Calc.php"],
    deps = ["//base:base"],
)
"#;

const BASE_BUILD: &str = r#"load("@rules_dx//php/rules:defs.bzl", "php_library")

php_library(
    name = "base",
    srcs = ["src/Doubler.php"],
    visibility = ["//visibility:public"],
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("php/rules/tool.php");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer(source: &str, lib_build: &str) -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("lib/BUILD.bazel"), lib_build);
    write(&root.join("lib/src/Calc.php"), source);
    root
}

fn dx_with(root: &Case, tool: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![
        "typecheck",
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ];
    if tool {
        argv.push("--@rules_dx//config:tool_phpstan=@rules_dx//php/tools:phpstan");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case) -> Run {
    dx_with(root, true)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

#[test]
fn clean_library_passes() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn return_type_error_is_reported_at_its_line() {
    let root = consumer(BAD_RETURN, LIB_BUILD);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("lib/src/Calc.php:10") && out.stdout.contains("[phpstan/return.type]"),
        "{}",
        out.combined()
    );
    assert_eq!(read(&root.join("lib/src/Calc.php")), BAD_RETURN);
}

#[test]
fn dependency_classes_resolve_without_unknown_class_errors() {
    let root = consumer(CALLER, CALLER_BUILD);
    write(&root.join("base/BUILD.bazel"), BASE_BUILD);
    write(&root.join("base/src/Doubler.php"), BASE);
    let out = dx(&root);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer(CLEAN, LIB_BUILD);
    let out = dx_with(&root, false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_phpstan"),
        "{}",
        out.combined()
    );
}
