use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, serde_json, write, Run};

const CLEAN: &str = "def add(a: int, b: int) -> int:\n    return a + b\n";

const DIRTY_FORMAT: &str = "def add(a:int,b:int)->int:\n    return a+b\n";

const SHIM_EDIT: &str = "def add(a:int,b:int)->int:\n    return a - b  # edited during the run\n";

const DIRTY_LINT: &str = "import os\n\n\ndef add(a: int, b: int) -> int:\n    return a + b\n";

const FIXED_LINT: &str = "\n\ndef add(a: int, b: int) -> int:\n    return a + b\n";

const MODULE: &str = r#"module(name = "renamed_quality_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.75.0")
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

const BUILD: &str = r#"load("@dxr//quality:native_config.bzl", "ruff_config")
load("@dxr//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "python_family",
    family_id = "python",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["ruff"],
    lint = ["ruff"],
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

const APP_BUILD: &str = r#"load("@dxr//python/rules:defs.bzl", "python_library")

python_library(
    name = "app",
    srcs = ["app.py"],
    aspect_hints = ["//:ruff"],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
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
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/app.py"), CLEAN);
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    dx_in(root, command, args, None)
}

fn dx_in(root: &Case, command: &str, args: &[&str], shim: Option<&Path>) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("ruff-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@dxr//config:workspace=//:policy",
    ]);
    let envs = [("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")];
    match shim {
        Some(dir) => root.run_with_shim(&resolve_runfiles(&rel), &argv, &envs, dir),
        None => root.run(&resolve_runfiles(&rel), &argv, &envs),
    }
}

/// Writes a `bazel` shim that runs the real one, then once, after the main build succeeds,
/// overwrites `target` with `SHIM_EDIT` and logs the arguments. It keeps bazel's exit code.
fn stale_shim(dir: &Path, real: &Path, target: &Path, sentinel: &Path, log: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let q = nested::shell_quote;
    let edit = dir.join("edit.txt");
    write(&edit, SHIM_EDIT);
    let script = format!(
        "#!/bin/sh\n{real} \"$@\"\ncode=$?\nif [ \"$code\" = 0 ] && [ ! -e {sentinel} ]; then\n  case \" $* \" in\n    *' --output_groups=dx_results '*)\n      : > {sentinel}\n      cat {edit} > {target}\n      echo \"$*\" >> {log}\n      ;;\n  esac\nfi\nexit $code\n",
        real = q(&real.to_string_lossy()),
        sentinel = q(&sentinel.to_string_lossy()),
        edit = q(&edit.to_string_lossy()),
        target = q(&target.to_string_lossy()),
        log = q(&log.to_string_lossy()),
    );
    let path = dir.join("bazel");
    write(&path, &script);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod shim: {error}"));
    path
}

fn events(run: &Run) -> Vec<serde_json::Value> {
    let out = run.stdout.clone();
    out.lines()
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

fn kinds(events: &[serde_json::Value]) -> Vec<String> {
    events
        .iter()
        .map(|event| event["event"].as_str().unwrap_or("?").to_string())
        .collect()
}

#[test]
fn clean_json_runs_have_a_constant_event_count() {
    let root = consumer();
    let first = dx(&root, "lint", &["--output=json"]);
    assert_eq!(first.status.code(), Some(0), "{}", first.combined());
    let again = dx(&root, "lint", &["--output=json"]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
    let a = events(&first);
    let b = events(&again);
    assert_eq!(kinds(&a), kinds(&b), "{:?}", kinds(&a));
    assert_eq!(
        a.first().map(|e| e["event"].clone()),
        Some("command_started".into())
    );
    assert_eq!(
        a.last().map(|e| e["event"].clone()),
        Some("command_finished".into())
    );

    write(&root.join("app/extra.py"), CLEAN);
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("[\"app.py\"]", "[\"app.py\", \"extra.py\"]"),
    );
    let more = dx(&root, "lint", &["--output=json"]);
    assert_eq!(more.status.code(), Some(0), "{}", more.combined());
    assert_eq!(events(&more).len(), a.len(), "{:?}", kinds(&events(&more)));
    assert_eq!(a.len(), 2, "{:?}", kinds(&a));
    assert!(first.stdout.len() < 400, "{} bytes", first.stdout.len());
}

#[test]
fn dirty_format_check_reports_compact_changes_and_edits_only_on_request() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, DIRTY_FORMAT);

    let compact = dx(&root, "format", &["--output=json"]);
    assert_eq!(compact.status.code(), Some(1), "{}", compact.combined());
    let full = dx(&root, "format", &["--output=json", "--include-edits"]);
    assert_eq!(full.status.code(), Some(1), "{}", full.combined());
    assert_eq!(read(&file), DIRTY_FORMAT);
    assert!(
        compact.stdout.len() < full.stdout.len(),
        "compact {} full {}",
        compact.stdout.len(),
        full.stdout.len()
    );
    let compact_text = compact.stdout.clone();
    assert!(compact_text.contains("app/app.py"), "{compact_text}");
    assert!(!compact_text.contains("a: int, b: int"), "{compact_text}");
    let compact_changes = of_kind(&events(&compact), "change");
    assert_eq!(compact_changes.len(), 1, "{compact_text}");
    let compact_change = &compact_changes[0];
    assert_eq!(compact_change["path"], "app/app.py");
    assert_eq!(compact_change["effect"], "proposed");
    assert!(compact_change["edit_count"].as_u64().unwrap_or(0) >= 1);
    assert!(compact_change.get("edits").is_none(), "{compact_text}");
    let full_changes = of_kind(&events(&full), "change");
    assert_eq!(full_changes.len(), 1, "{}", full.stdout);
    let full_change = &full_changes[0];
    assert_eq!(full_change["edit_count"], compact_change["edit_count"]);
    let edits = full_change["edits"]
        .as_array()
        .unwrap_or_else(|| panic!("{full_change}"));
    assert_eq!(
        edits.len() as u64,
        full_change["edit_count"].as_u64().unwrap_or(0)
    );
    assert!(
        edits.iter().any(|edit| edit["replacement"]
            .as_str()
            .is_some_and(|text| !text.is_empty())),
        "{full_change}"
    );
    assert_eq!(events(&compact).len(), 4, "{} bytes", compact.stdout.len());
    assert!(
        compact.stdout.len() < 1000,
        "{} bytes",
        compact.stdout.len()
    );

    let rejected = dx(&root, "format", &["--include-edits"]);
    assert_eq!(rejected.status.code(), Some(2), "{}", rejected.combined());
    assert!(
        rejected
            .combined()
            .contains("option \"--include-edits\" requires --output=json"),
        "{}",
        rejected.combined()
    );
    assert_eq!(read(&file), DIRTY_FORMAT);
}

#[test]
fn dirty_format_apply_rewrites_and_reports_the_effect() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, DIRTY_FORMAT);
    let applied = dx(&root, "format", &["--apply", "--output=json"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&file), CLEAN);
    let text = applied.stdout.clone();
    assert!(text.contains("app/app.py"), "{text}");
    let changes = of_kind(&events(&applied), "change");
    assert_eq!(changes.len(), 1, "{text}");
    assert_eq!(changes[0]["path"], "app/app.py");
    assert_eq!(changes[0]["kind"], "modify");
    assert_eq!(changes[0]["effect"], "applied");
    assert!(
        changes[0]["edit_count"].as_u64().unwrap_or(0) >= 1,
        "{text}"
    );
    assert_eq!(events(&applied).len(), 3, "{} bytes", applied.stdout.len());
    let again = dx(&root, "format", &["--output=json"]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

#[test]
fn format_apply_refuses_a_source_edited_during_the_run() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, DIRTY_FORMAT);
    let dir_guard = scratch("ruff-shim");
    let dir = dir_guard.path();
    let sentinel = dir.join("fired");
    let log = dir.join("calls.log");
    stale_shim(dir, &root.wrapper(), &file, &sentinel, &log);
    let run = dx_in(&root, "format", &["--apply", "--output=json"], Some(dir));
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert!(sentinel.exists(), "{}", run.combined());
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(calls.lines().count(), 1, "{calls}");
    assert_eq!(read(&file), SHIM_EDIT);
    let all = events(&run);
    let changes = of_kind(&all, "change");
    assert_eq!(changes.len(), 1, "{}", run.stdout);
    assert_eq!(changes[0]["path"], "app/app.py");
    assert_eq!(changes[0]["effect"], "refused");
    assert_eq!(changes[0]["reason"], "stale_source");
    assert!(of_kind(&all, "mutation").is_empty(), "{}", run.stdout);
    let finished = of_kind(&all, "command_finished");
    assert_eq!(finished[0]["exit_code"], 1, "{}", run.stdout);
}

#[test]
fn dirty_lint_reports_rule_and_position_and_apply_fixes_it() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, DIRTY_LINT);
    let dirty = dx(&root, "lint", &["--output=json"]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    let text = dirty.stdout.clone();
    assert!(text.contains("F401"), "{text}");
    assert!(text.contains("app/app.py"), "{text}");
    let findings = of_kind(&events(&dirty), "diagnostic");
    assert_eq!(findings.len(), 1, "{text}");
    let finding = &findings[0];
    assert_eq!(finding["tool"], "ruff");
    assert_eq!(finding["rule"], "F401");
    assert_eq!(finding["path"], "app/app.py");
    assert_eq!(finding["range"]["start_line"], 1, "{text}");
    assert_eq!(finding["range"]["start_column"], 8, "{text}");
    assert_eq!(read(&file), DIRTY_LINT);
    assert_eq!(events(&dirty).len(), 4, "{} bytes", dirty.stdout.len());

    let applied = dx(&root, "lint", &["--apply", "--output=json"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert_eq!(read(&file), FIXED_LINT);
    let again = dx(&root, "lint", &["--output=json"]);
    assert_eq!(again.status.code(), Some(0), "{}", again.combined());
}

const SHIFTING: &str =
    "import os\n\n\ndef add(a: int, b: int) -> int:\n    return a + b + missing\n";

const SHIFTED: &str = "\n\ndef add(a: int, b: int) -> int:\n    return a + b + missing\n";

const UNICODE_CRLF: &str = "# h\u{e9}llo \u{2713}\r\nimport os\r\n\r\n\r\ndef add(a: int, b: int) -> int:\r\n    return a + b + missing\r\n";

const UNICODE_EOF: &str = "import os\r\nx = \"h\u{e9}llo \u{1f600}\"; y = missing";

const UNICODE_EOF_FIXED: &str = "x = \"h\u{e9}llo \u{1f600}\"; y = missing";

fn sarif_results(root: &Case, name: &str) -> Vec<serde_json::Value> {
    let text = read(&root.join(name));
    let document: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("sarif {text:?}: {error}"));
    document["runs"][0]["results"]
        .as_array()
        .unwrap_or_else(|| panic!("{text}"))
        .clone()
}

fn region(result: &serde_json::Value) -> (u64, u64) {
    let region = &result["locations"][0]["physicalLocation"]["region"];
    (
        region["startLine"].as_u64().unwrap_or(0),
        region["startColumn"].as_u64().unwrap_or(0),
    )
}

fn rule_of(result: &serde_json::Value) -> &str {
    result["ruleId"].as_str().unwrap_or("")
}

fn single_region(results: &[serde_json::Value], rule: &str) -> (u64, u64) {
    let matching: Vec<&serde_json::Value> = results
        .iter()
        .filter(|result| rule_of(result) == rule)
        .collect();
    assert_eq!(matching.len(), 1, "{results:?}");
    region(matching[0])
}

#[test]
fn check_writes_sarif_at_original_locations_when_the_fix_shifts_lines() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, SHIFTING);
    let run = dx(
        &root,
        "lint",
        &["--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), SHIFTING);
    let results = sarif_results(&root, "out.sarif");
    let unused = results
        .iter()
        .find(|result| rule_of(result) == "F401")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(unused), (1, 8));
    let missing = results
        .iter()
        .find(|result| rule_of(result) == "F821")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(missing), (5, 20));
}

#[test]
fn apply_reports_the_remaining_finding_at_its_final_location() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, SHIFTING);
    let run = dx(
        &root,
        "lint",
        &["--apply", "--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), SHIFTED);
    let results = sarif_results(&root, "out.sarif");
    assert!(
        results.iter().all(|result| rule_of(result) != "F401"),
        "{results:?}"
    );
    assert_eq!(single_region(&results, "F821"), (4, 20));
    let again = dx(
        &root,
        "lint",
        &["--output=json", "--report=sarif=again.sarif"],
    );
    assert_eq!(again.status.code(), Some(1), "{}", again.combined());
    assert_eq!(read(&file), SHIFTED);
    assert_eq!(
        single_region(&sarif_results(&root, "again.sarif"), "F821"),
        (4, 20)
    );
}

#[test]
fn check_sarif_is_correct_for_non_ascii_crlf_sources() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, UNICODE_CRLF);
    let run = dx(
        &root,
        "lint",
        &["--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), UNICODE_CRLF);
    let results = sarif_results(&root, "out.sarif");
    let unused = results
        .iter()
        .find(|result| rule_of(result) == "F401")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(unused), (2, 8));
    let missing = results
        .iter()
        .find(|result| rule_of(result) == "F821")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(missing), (6, 20));
}

#[test]
fn check_sarif_columns_count_characters_after_a_non_ascii_prefix_without_final_newline() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, UNICODE_EOF);
    let run = dx(
        &root,
        "lint",
        &["--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), UNICODE_EOF);
    let results = sarif_results(&root, "out.sarif");
    let unused = results
        .iter()
        .find(|result| rule_of(result) == "F401")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(unused), (1, 8));
    let missing = results
        .iter()
        .find(|result| rule_of(result) == "F821")
        .unwrap_or_else(|| panic!("{results:?}"));
    assert_eq!(region(missing), (2, 20));
}

#[test]
fn apply_on_non_ascii_crlf_source_without_final_newline_writes_exact_bytes() {
    let root = consumer();
    let file = root.join("app/app.py");
    write(&file, UNICODE_EOF);
    let run = dx(
        &root,
        "lint",
        &["--apply", "--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), UNICODE_EOF_FIXED);
    let results = sarif_results(&root, "out.sarif");
    assert!(
        results.iter().all(|result| rule_of(result) != "F401"),
        "{results:?}"
    );
    assert_eq!(single_region(&results, "F821"), (1, 20));
    let again = dx(
        &root,
        "lint",
        &["--output=json", "--report=sarif=again.sarif"],
    );
    assert_eq!(again.status.code(), Some(1), "{}", again.combined());
    assert_eq!(read(&file), UNICODE_EOF_FIXED);
    assert_eq!(
        single_region(&sarif_results(&root, "again.sarif"), "F821"),
        (1, 20)
    );
}

#[test]
fn nondefault_rule_selection_limits_findings_and_apply_leaves_unselected_code() {
    let root = consumer();
    write(&root.join("ruff.toml"), "[lint]\nselect = [\"F821\"]\n");
    let file = root.join("app/app.py");
    write(&file, SHIFTING);
    let run = dx(
        &root,
        "lint",
        &["--apply", "--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(run.status.code(), Some(1), "{}", run.combined());
    assert_eq!(read(&file), SHIFTING);
    let results = sarif_results(&root, "out.sarif");
    assert!(
        results.iter().all(|result| rule_of(result) == "F821"),
        "{results:?}"
    );
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(region(&results[0]), (5, 20));
}
