use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_testing::nested::{self, Case};
use dx_testing::{mkscratch, resolve_runfiles, serde_json, Run};

const CLEAN: &str = "def add(a: int, b: int) -> int:\n    return a + b\n";

const DIRTY_FORMAT: &str = "def add(a:int,b:int)->int:\n    return a+b\n";

const DIRTY_LINT: &str = "import os\n\n\ndef add(a: int, b: int) -> int:\n    return a + b\n";

const MODULE: &str = r#"module(name = "renamed_quality_consumer")

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

const BUILD: &str = r#"load("@rules_dx//quality:native_config.bzl", "ruff_config")
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

const RUFF_TOML: &str = "line-length = 88\n";

fn library(name: &str) -> String {
    format!(
        "python_library(\n    name = \"{name}\",\n    srcs = [\"{name}.py\"],\n    aspect_hints = [\"//:ruff\"],\n)\n"
    )
}

fn package(names: &[&str]) -> String {
    let libraries: Vec<String> = names.iter().map(|name| library(name)).collect();
    format!(
        "load(\"@rules_dx//python/rules:defs.bzl\", \"python_library\")\n\n{}",
        libraries.join("\n")
    )
}

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
    let marker = resolve_runfiles("quality/tests/check_continuation_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

const TYPE_ERROR: &str = "def total() -> int:\n    return \"x\"\n";

const DX_BUILD: &str = r#"alias(
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

struct Sources {
    app: &'static str,
    fmt: &'static str,
    lint: &'static str,
    typed: &'static str,
    drift: bool,
}

const CLEAN_SOURCES: Sources = Sources {
    app: CLEAN,
    fmt: CLEAN,
    lint: CLEAN,
    typed: CLEAN,
    drift: false,
};

const DRIFT_SOURCES: Sources = Sources {
    app: CLEAN,
    fmt: DIRTY_FORMAT,
    lint: DIRTY_LINT,
    typed: TYPE_ERROR,
    drift: true,
};

fn consumer(sources: &Sources) -> Case {
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

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
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

fn assert_same(root: &Path, before: &BTreeMap<String, Vec<u8>>) {
    let after = snapshot(root);
    let changed: Vec<&String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .collect();
    assert!(changed.is_empty(), "check changed {changed:?}");
}

fn dx(root: &Case, command: &str, scope: &str, args: &[&str]) -> Run {
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

fn events(run: &Run) -> Vec<serde_json::Value> {
    run.stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("json {line:?}: {error}"))
        })
        .collect()
}

fn of_kind(events: &[serde_json::Value], kind: &str) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter(|event| event["event"] == kind)
        .cloned()
        .collect()
}

fn phase_exits(events: &[serde_json::Value]) -> Vec<(String, i64)> {
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

fn exits(format: i64, lint: i64, typecheck: i64, generate: i64) -> Vec<(String, i64)> {
    vec![
        ("format".to_owned(), format),
        ("lint".to_owned(), lint),
        ("typecheck".to_owned(), typecheck),
        ("generate".to_owned(), generate),
    ]
}

fn finished(events: &[serde_json::Value]) -> serde_json::Value {
    events
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("no events"))
}

#[test]
fn one_check_returns_every_independent_finding_and_leaves_bytes_unchanged() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(1, 1, 1, 1), "{}", run.stdout);
    assert_eq!(of_kind(&all, "phase_started").len(), 4, "{}", run.stdout);
    let last = finished(&all);
    assert_eq!(last["event"], "command_finished", "{}", run.stdout);
    assert_eq!(last["exit_code"], 1);
    assert_eq!(last["results_complete"], true, "{}", run.stdout);
    let rules: Vec<(String, String, String)> = of_kind(&all, "diagnostic")
        .iter()
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["tool"].as_str().unwrap_or("?").to_owned(),
                event["rule"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    assert_eq!(
        rules,
        vec![
            (
                "check/format".to_owned(),
                "ruff".to_owned(),
                "unformatted".to_owned()
            ),
            (
                "check/lint".to_owned(),
                "ruff".to_owned(),
                "F401".to_owned()
            ),
            (
                "check/typecheck".to_owned(),
                "ty".to_owned(),
                "invalid-return-type".to_owned()
            ),
        ],
        "{}",
        run.stdout
    );
    let changes: Vec<(String, String)> = of_kind(&all, "change")
        .iter()
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["path"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    assert!(
        changes.contains(&("check/generate".to_owned(), "gen/BUILD.bazel".to_owned())),
        "{changes:?}"
    );
    assert!(
        of_kind(&all, "change")
            .iter()
            .all(|event| event["effect"] == "proposed"),
        "{}",
        run.stdout
    );
    let noticed: Vec<String> = of_kind(&all, "notice")
        .iter()
        .filter(|event| event["code"] == "umbrella_phase_findings")
        .map(|event| event["correlation"].as_str().unwrap_or("?").to_owned())
        .collect();
    assert_eq!(
        noticed,
        [
            "check/format",
            "check/lint",
            "check/typecheck",
            "check/generate"
        ],
        "{}",
        run.stdout
    );
    assert!(
        of_kind(&all, "notice")
            .iter()
            .all(|event| event["code"] != "umbrella_phase_skipped"),
        "{}",
        run.stdout
    );
    assert_same(&root, &before);
}

#[test]
fn text_and_report_views_agree_with_the_phase_outcomes() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let text = dx(&root, "check", "//...", &["--output=text"]);
    assert_eq!(text.status.code(), Some(1), "{}", text.combined());
    assert!(
        text.stderr.contains(
            "dx: phases: format findings, lint findings, typecheck findings, generate findings"
        ),
        "{}",
        text.combined()
    );
    let reported = dx(
        &root,
        "check",
        "//...",
        &["--output=json", "--report", "sarif=check.sarif"],
    );
    assert_eq!(reported.status.code(), Some(1), "{}", reported.combined());
    let all = events(&reported);
    assert_eq!(phase_exits(&all), exits(1, 1, 1, 1), "{}", reported.stdout);
    let sarif: serde_json::Value = serde_json::from_str(&read(&root.join("check.sarif")))
        .unwrap_or_else(|error| panic!("sarif: {error}"));
    assert_eq!(sarif["version"], "2.1.0");
    let results: Vec<String> = sarif["runs"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .flat_map(|run| run["results"].as_array().cloned().unwrap_or_default())
        .map(|result| result["ruleId"].as_str().unwrap_or("?").to_owned())
        .collect();
    assert!(
        results.iter().any(|rule| rule.ends_with("F401")),
        "{results:?}"
    );
    assert!(
        results
            .iter()
            .any(|rule| rule.ends_with("invalid-return-type")),
        "{results:?}"
    );
    let report = of_kind(&all, "report");
    let written = report
        .iter()
        .find(|event| event["path"] == "check.sarif")
        .unwrap_or_else(|| panic!("{}", reported.stdout));
    assert_eq!(written["results_complete"], true, "{}", reported.stdout);
    let mut expected = before;
    expected.insert(
        "check.sarif".to_owned(),
        read(&root.join("check.sarif")).into_bytes(),
    );
    assert_same(&root, &expected);
}

#[test]
fn a_later_clean_phase_never_clears_an_earlier_finding() {
    let root = consumer(&Sources {
        fmt: DIRTY_FORMAT,
        ..CLEAN_SOURCES
    });
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(1, 0, 0, 0), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 1);
    assert_eq!(finished(&all)["results_complete"], true);
    assert_same(&root, &before);
}

#[test]
fn generation_drift_alone_fails_after_three_clean_phases() {
    let root = consumer(&Sources {
        drift: true,
        ..CLEAN_SOURCES
    });
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(0, 0, 0, 1), "{}", run.stdout);
    assert_eq!(of_kind(&all, "diagnostic").len(), 0, "{}", run.stdout);
    assert_eq!(of_kind(&all, "change").len(), 1, "{}", run.stdout);
    assert_same(&root, &before);
}

#[test]
fn a_clean_consumer_passes_every_phase() {
    let root = consumer(&CLEAN_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "//...", &["--output=json"]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let all = events(&run);
    assert_eq!(phase_exits(&all), exits(0, 0, 0, 0), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 0);
    assert_eq!(finished(&all)["results_complete"], true);
    assert!(of_kind(&all, "diagnostic").is_empty(), "{}", run.stdout);
    assert_same(&root, &before);
}

#[test]
fn a_rejected_scope_stops_before_any_later_phase() {
    let root = consumer(&DRIFT_SOURCES);
    let before = snapshot(&root);
    let run = dx(&root, "check", "no/such/dir", &["--output=json"]);
    assert_eq!(run.status.code(), Some(2), "{}", run.combined());
    let all = events(&run);
    assert_eq!(of_kind(&all, "phase_started").len(), 1, "{}", run.stdout);
    assert_eq!(
        phase_exits(&all),
        vec![("format".to_owned(), 2)],
        "{}",
        run.stdout
    );
    let errors = of_kind(&all, "error");
    assert_eq!(errors.len(), 1, "{}", run.stdout);
    assert_eq!(errors[0]["code"], "invalid_scope", "{}", run.stdout);
    let skipped: Vec<(String, String)> = of_kind(&all, "notice")
        .iter()
        .filter(|event| event["code"] == "umbrella_phase_skipped")
        .map(|event| {
            (
                event["correlation"].as_str().unwrap_or("?").to_owned(),
                event["message"].as_str().unwrap_or("?").to_owned(),
            )
        })
        .collect();
    let expected: Vec<(String, String)> = ["lint", "typecheck", "generate"]
        .iter()
        .map(|phase| {
            (
                format!("check/{phase}"),
                format!("{phase} skipped after format rejected its scope or configuration"),
            )
        })
        .collect();
    assert_eq!(skipped, expected, "{}", run.stdout);
    assert!(of_kind(&all, "diagnostic").is_empty(), "{}", run.stdout);
    assert_eq!(finished(&all)["exit_code"], 2, "{}", run.stdout);
    assert_eq!(finished(&all)["results_complete"], false, "{}", run.stdout);
    assert_same(&root, &before);
}
