use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "platforms", version = "1.1.0")
bazel_dep(name = "rules_rust", version = "0.74.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")
bazel_dep(name = "rules_python", version = "RULES_PYTHON")
bazel_dep(name = "aspect_rules_py", version = "ASPECT_RULES_PY")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")

uv = use_extension("@aspect_rules_py//uv:extensions.bzl", "uv")
uv.declare_hub(hub_name = "shop_pypi")
uv.project(
    hub_name = "shop_pypi",
    lock = "//:uv.lock",
    pyproject = "//:pyproject.toml",
)
use_repo(uv, "shop_pypi")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    artifacts = [],
    known_contributing_modules = ["protobuf", "rules_dx"],
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven")
"#;

const ROOT_BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "ruff_config")

ruff_config(
    name = "ruff_config",
    src = "ruff.toml",
    visibility = ["//visibility:public"],
)
"#;

const NEGATIVE_BUILD: &str = r#"load("@rules_dx//python/rules:defs.bzl", "python_library", "python_test")

python_library(
    name = "lint_probe",
    srcs = ["lint_probe.py"],
    aspect_hints = ["//:ruff_config"],
    imports = [".."],
)

python_library(
    name = "type_probe",
    srcs = ["type_probe.py"],
    imports = [".."],
)

python_test(
    name = "syntax_test",
    srcs = ["syntax_test.py"],
    dep_group = "ledger",
    imports = [".."],
    deps = [
        "@shop_pypi//coverage",
        "@shop_pypi//pytest",
    ],
)

python_test(
    name = "failing_test",
    srcs = ["failing_test.py"],
    dep_group = "ledger",
    imports = [".."],
    deps = [
        "@shop_pypi//coverage",
        "@shop_pypi//pytest",
    ],
)

python_test(
    name = "undeclared_test",
    srcs = ["undeclared_test.py"],
    dep_group = "ledger",
    imports = [".."],
    deps = [
        "@shop_pypi//coverage",
        "@shop_pypi//pytest",
    ],
)
"#;

const TYPE_PROBE: &str = "\"\"\"Probe.\"\"\"\n\n\ndef double(value: int) -> int:\n    \"\"\"Double the value.\"\"\"\n    return value * 2\n\n\nWRONG: str = double(\"two\")\n";

const SYNTAX_TEST: &str = "def test_never_runs(:\n    pass\n";

const FAILING_TEST: &str = "def test_fails():\n    assert 1 + 1 == 3\n";

const UNDECLARED_TEST: &str = "import yaml\n\n\ndef test_never_runs():\n    assert yaml\n";

const LINT_PROBE: &str = "\"\"\"Probe.\"\"\"\n\nimport os\n";

const SOURCES: [&str; 3] = ["core", "service", "cli"];

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("python/tests/consumer/service_consumer_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing from versions.bzl"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn copy_source(from: &str, to: &Path) {
    write(to, &read(&resolve_runfiles(from)));
}

fn consumer_build(example: &str) -> String {
    let without_fixtures = example
        .lines()
        .filter(|line| !line.contains("fixtures.bzl"))
        .collect::<Vec<_>>()
        .join("\n");
    let kept = without_fixtures
        .split("\n\n")
        .filter(|block| {
            !block.starts_with("real_source_target(") && !block.contains("name = \"consumer_srcs\"")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let build = kept
        .replace(
            "//examples/python-service:__subpackages__",
            "//:__subpackages__",
        )
        .replace("//examples/python-service/ledger/", "//ledger/")
        .replace("@pypi//", "@shop_pypi//")
        .trim_end()
        .to_owned()
        + "\n";
    for line in build.lines().filter(|line| line.starts_with("load(")) {
        assert!(
            line.starts_with("load(\"@rules_dx//"),
            "copied example BUILD loads a non-public label: {line}"
        );
    }
    build
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let versions = read(&rules.join("modules/versions.bzl"));
    let case = nested::case("consumer");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_PYTHON", &pin(&versions, "FLOOR_RULES_PYTHON"))
            .replace(
                "ASPECT_RULES_PY",
                &pin(&versions, "FLOOR_ASPECT_RULES_PY"),
            ),
    );
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&rules));
    write(&root.join("BUILD.bazel"), ROOT_BUILD);
    write(&root.join("ruff.toml"), "[lint]\nselect = [\"E\", \"F\"]\n");
    for file in ["pyproject.toml", "uv.lock"] {
        copy_source(&format!("examples/python-service/{file}"), &root.join(file));
    }
    for package in SOURCES {
        let dir = root.join("ledger").join(package);
        let source = rules.join("examples/python-service/ledger").join(package);
        for entry in
            std::fs::read_dir(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"))
        {
            let name = entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            let from = format!("examples/python-service/ledger/{package}/{name}");
            if name == "BUILD.bazel" {
                write(
                    &dir.join(&name),
                    &consumer_build(&read(&resolve_runfiles(&from))),
                );
            } else if name.ends_with(".py") || name.ends_with(".json") {
                copy_source(&from, &dir.join(&name));
            }
        }
    }
    write(&root.join("ledger/__init__.py"), "");
    write(&root.join("negative/BUILD.bazel"), NEGATIVE_BUILD);
    write(&root.join("negative/syntax_test.py"), SYNTAX_TEST);
    write(&root.join("negative/lint_probe.py"), LINT_PROBE);
    write(&root.join("negative/type_probe.py"), TYPE_PROBE);
    write(&root.join("negative/failing_test.py"), FAILING_TEST);
    write(&root.join("negative/undeclared_test.py"), UNDECLARED_TEST);
    case
}

struct Outcome {
    code: Option<i32>,
    stdout: String,
    text: String,
}

fn outcome(run: Run) -> Outcome {
    Outcome {
        code: run.status.code(),
        text: run.combined(),
        stdout: run.stdout,
    }
}

fn bazel(case: &Case, verb: &str, flags: &[&str], targets: &[&str]) -> Outcome {
    let mut args = vec![verb, "--color=no", "--curses=no"];
    args.extend_from_slice(flags);
    args.extend_from_slice(targets);
    outcome(case.bazel(&args))
}

fn ledger(case: &Case, args: &[&str], envs: &[(&str, &str)]) -> Outcome {
    let mut argv = vec![
        "run",
        "--color=no",
        "--curses=no",
        "//ledger/cli:ledger",
        "--",
    ];
    argv.extend_from_slice(args);
    outcome(case.bazel_with(&argv, envs))
}

fn sha256_hex(path: &Path) -> String {
    dx_testing::sha256_file(path).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn every_test_of_the_application_passes_under_both_runners() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        &["--test_output=errors"],
        &[
            "//ledger/core:all",
            "//ledger/service:all",
            "//ledger/cli:all",
        ],
    );
    assert_eq!(tested.code, Some(0), "{}", tested.text);
    for name in [
        "//ledger/core:catalog_test",
        "//ledger/core:build_info_test",
        "//ledger/service:server_test",
        "//ledger/cli:cli_test",
        "//ledger/cli:cli_unittest_test",
        "//ledger/cli:broken_test",
    ] {
        let line = tested
            .text
            .lines()
            .find(|line| line.starts_with(name))
            .unwrap_or_else(|| panic!("{name} did not run:\n{}", tested.text));
        assert!(line.contains("PASSED"), "{line}");
    }
}

#[test]
fn the_binary_serves_data_and_generated_identity_from_the_consumer_workspace() {
    let case = consumer();
    let listed = ledger(&case, &["list"], &[]);
    assert_eq!(listed.code, Some(0), "{}", listed.text);
    assert_eq!(
        listed.stdout.lines().collect::<Vec<_>>(),
        [
            "bolt-m6\tBolt M6\t45",
            "nut-m6\tNut M6\t20",
            "washer-m6\tWasher M6\t8"
        ]
    );
    let info = ledger(&case, &["info"], &[]);
    assert_eq!(info.code, Some(0), "{}", info.text);
    let digest = sha256_hex(&case.join("ledger/core/items.json"));
    assert!(
        info.stdout.contains(&format!("sha256 {digest}")),
        "{}",
        info.text
    );
    assert!(info.stdout.contains("items 3"), "{}", info.text);
}

#[test]
fn a_config_file_overrides_the_defaults_and_a_bad_one_is_rejected() {
    let case = consumer();
    let dir = mkscratch("python-service-config").unwrap_or_else(|error| panic!("scratch: {error}"));
    let good = dir.join("ledger.json");
    write(&good, "{\"currency\": \"EUR\", \"tax_percent\": 10}\n");
    let path = good.to_string_lossy().into_owned();
    let plain = ledger(&case, &["quote", "bolt-m6", "--qty", "4"], &[]);
    let taxed = ledger(
        &case,
        &["--config", &path, "quote", "bolt-m6", "--qty", "4"],
        &[],
    );
    let from_env = ledger(
        &case,
        &["quote", "bolt-m6", "--qty", "4"],
        &[("LEDGER_CONFIG", &path)],
    );
    assert_eq!(plain.code, Some(0), "{}", plain.text);
    assert_eq!(taxed.code, Some(0), "{}", taxed.text);
    assert_ne!(plain.stdout, taxed.stdout);
    assert!(taxed.stdout.contains("EUR"), "{}", taxed.text);
    assert_eq!(taxed.stdout, from_env.stdout);

    let bad = dir.join("bad.json");
    write(&bad, "{\"colour\": \"red\"}\n");
    let rejected = ledger(&case, &["--config", &bad.to_string_lossy(), "list"], &[]);
    assert_ne!(rejected.code, Some(0), "{}", rejected.text);
    assert!(
        rejected.text.contains("unknown config keys: colour"),
        "{}",
        rejected.text
    );
}

#[test]
fn coverage_covers_the_application_sources() {
    let case = consumer();
    let covered = bazel(
        &case,
        "coverage",
        &["--combined_report=lcov"],
        &["//ledger/core:catalog_test"],
    );
    assert_eq!(covered.code, Some(0), "{}", covered.text);
    let info = bazel(&case, "info", &["output_path"], &[]);
    let report = PathBuf::from(info.stdout.trim()).join("_coverage/_coverage_report.dat");
    let lcov = read(&report);
    let record = lcov
        .split("end_of_record")
        .find(|record| record.contains("SF:ledger/core/catalog.py"))
        .unwrap_or_else(|| panic!("catalog.py missing from coverage:\n{lcov}"));
    let hit = record
        .lines()
        .filter_map(|line| line.strip_prefix("LH:"))
        .any(|count| count.trim().parse::<u32>().is_ok_and(|count| count > 0));
    assert!(hit, "no catalog.py line was hit:\n{record}");
}

#[test]
fn a_syntax_error_fails_its_test_with_the_diagnostic() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        &["--test_output=all"],
        &["//negative:syntax_test"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(tested.text.contains("SyntaxError"), "{}", tested.text);
}

#[test]
fn a_failing_assertion_fails_the_test() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        &["--test_output=all"],
        &["//negative:failing_test"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(tested.text.contains("assert 1 + 1 == 3"), "{}", tested.text);
}

#[test]
fn an_undeclared_dependency_is_not_resolved_from_the_host() {
    let case = consumer();
    let tested = bazel(
        &case,
        "test",
        &["--test_output=all"],
        &["//negative:undeclared_test"],
    );
    assert_eq!(tested.code, Some(3), "{}", tested.text);
    assert!(
        tested.text.contains("No module named 'yaml'"),
        "{}",
        tested.text
    );
}

fn dx_bin() -> PathBuf {
    let dx = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&dx)
}

fn typecheck(case: &Case, target: &str) -> Run {
    let workspace = case.to_string_lossy().into_owned();
    case.run(
        &dx_bin(),
        &["typecheck", "--workspace", &workspace, target],
        &[],
    )
}

fn lint(case: &Case, target: &str) -> Run {
    let workspace = case.to_string_lossy().into_owned();
    case.run(&dx_bin(), &["lint", "--workspace", &workspace, target], &[])
}

#[test]
fn lint_passes_the_application_sources() {
    let case = consumer();
    let out = lint(&case, "//ledger/core:core");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn lint_reports_an_unused_import_at_its_position() {
    let case = consumer();
    let out = lint(&case, "//negative:lint_probe");
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("negative/lint_probe.py:3:"),
        "{}",
        out.combined()
    );
    assert!(out.combined().contains("F401"), "{}", out.combined());
}

#[test]
fn typecheck_passes_the_application_sources() {
    let case = consumer();
    let out = typecheck(&case, "//ledger/core:core");
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn typecheck_reports_a_type_error_at_its_position() {
    let case = consumer();
    let out = typecheck(&case, "//negative:type_probe");
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.combined().contains("negative/type_probe.py:9:"),
        "{}",
        out.combined()
    );
}

#[test]
fn init_adopts_an_existing_python_tree() {
    let dir = mkscratch("python-service-adopt").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &dir.join("pyproject.toml"),
        "[project]\nname = \"shop\"\nversion = \"0.1.0\"\n",
    );
    write(&dir.join("MODULE.bazel"), "module(name = \"shop\")\n");
    write(&dir.join("app/main.py"), "print(\"hi\")\n");
    let invoke = |args: &[&str]| {
        Command::new(dx_bin())
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run dx: {error}"))
    };
    let checked = invoke(&["init", "shop"]);
    assert_eq!(checked.status.code(), Some(1), "{checked:?}");
    assert!(String::from_utf8_lossy(&checked.stderr).contains("init drift"));
    assert!(!dir.join(".dx/version").exists());
    let applied = invoke(&["init", "--apply", "--with=github-ci", "shop"]);
    assert_eq!(applied.status.code(), Some(0), "{applied:?}");
    for file in [
        ".dx/version",
        "MODULE.bazel.snippet",
        ".github/workflows/ci.yml",
    ] {
        assert!(dir.join(file).is_file(), "{file} missing");
    }
    assert_eq!(read(&dir.join("app/main.py")), "print(\"hi\")\n");
}
