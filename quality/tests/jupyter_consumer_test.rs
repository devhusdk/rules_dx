use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, serde_json, write, Run};

const MODULE: &str = r#"module(name = "renamed_quality_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
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

const BUILD: &str = r#"load("@dxr//quality:native_config.bzl", "ruff_config")
load("@dxr//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "jupyter_family",
    family_id = "jupyter",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["ruff"],
    lint = [
        "ruff",
        "notebook_outputs",
    ],
)

workspace_policy(
    name = "policy",
    families = [":jupyter_family"],
)

ruff_config(
    name = "ruff",
    src = "ruff.toml",
    visibility = ["//visibility:public"],
)
"#;

const RUFF_TOML: &str = "line-length = 30\n\n[lint]\nselect = [\"F\"]\n";

const NB_BUILD: &str = r#"load("@dxr//jupyter/rules:defs.bzl", "jupyter_library")

jupyter_library(
    name = "notebooks",
    srcs = glob(["*.ipynb"]),
    aspect_hints = ["//:ruff"],
)
"#;

const DEFS: &str =
    "def compute_value(a, b):\n    return a + b\n\n\nfirst_argument = 1\nsecond_argument = 2";
const LONG_LINE: &str = "total = compute_value(first_argument, second_argument)";
const WRAPPED: &str = "total = compute_value(\n    first_argument,\n    second_argument,\n)";

/// One code cell: source text and whether it carries a saved output.
type Cell = (&'static str, bool);

fn notebook(cells: &[Cell]) -> String {
    let cells: Vec<serde_json::Value> = cells
        .iter()
        .enumerate()
        .map(|(index, (source, output))| {
            let lines: Vec<String> = source.split_inclusive('\n').map(str::to_string).collect();
            let outputs = if *output {
                serde_json::json!([{"name": "stdout", "output_type": "stream", "text": ["1\n"]}])
            } else {
                serde_json::json!([])
            };
            serde_json::json!({
                "cell_type": "code",
                "execution_count": null,
                "id": format!("c{index}"),
                "metadata": {},
                "outputs": outputs,
                "source": lines,
            })
        })
        .collect();
    let document = serde_json::json!({
        "cells": cells,
        "metadata": {"language_info": {"name": "python"}},
        "nbformat": 4,
        "nbformat_minor": 5,
    });
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_else(|e| panic!("{e}"));
    text.push('\n');
    text
}

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
    for package in ["alpha", "beta"] {
        write(&root.join(package).join("BUILD.bazel"), NB_BUILD);
        write(
            &root.join(package).join("clean.ipynb"),
            &notebook(&[(DEFS, false), ("print(first_argument)", false)]),
        );
    }
    write(
        &root.join("alpha/dirty.ipynb"),
        &notebook(&[(DEFS, false), ("import os", false), (LONG_LINE, false)]),
    );
    write(
        &root.join("beta/outputs.ipynb"),
        &notebook(&[(DEFS, false), ("print(first_argument)", true)]),
    );
    root
}

fn dx(root: &Case, command: &str, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("jupyter-home");
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
    root.run(&resolve_runfiles(&rel), &argv, &envs)
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

/// Diagnostics of `tool` reported for `path`.
fn findings(run: &Run, tool: &str, path: &str) -> Vec<serde_json::Value> {
    of_kind(&events(run), "diagnostic")
        .into_iter()
        .filter(|d| d["tool"] == tool && d["path"] == path)
        .collect()
}

/// The notebook text with every `"source": [...]` array replaced by a marker.
fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("\"source\": [") {
        out.push_str(&rest[..start]);
        out.push_str("SOURCE");
        let tail = &rest[start..];
        let close = tail
            .find(']')
            .unwrap_or_else(|| panic!("unterminated source"));
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

fn sources(text: &str) -> Vec<String> {
    let document: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|error| panic!("notebook {text:?}: {error}"));
    document["cells"]
        .as_array()
        .unwrap_or_else(|| panic!("{text}"))
        .iter()
        .map(|cell| {
            cell["source"]
                .as_array()
                .map(|lines| lines.iter().filter_map(|l| l.as_str()).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn notebooks_in_two_packages_are_checked_applied_and_rechecked() {
    let root = consumer();
    let dirty = root.join("alpha/dirty.ipynb");
    let outputs = root.join("beta/outputs.ipynb");
    let before_dirty = read(&dirty);
    let before_outputs = read(&outputs);
    let clean = [
        root.join("alpha/clean.ipynb"),
        root.join("beta/clean.ipynb"),
    ];
    let before_clean: Vec<String> = clean.iter().map(|p| read(p)).collect();

    let lint = dx(&root, "lint", &["--output=json"]);
    assert_eq!(lint.status.code(), Some(1), "{}", lint.combined());
    let unused = findings(&lint, "ruff", "alpha/dirty.ipynb");
    assert_eq!(unused.len(), 1, "{}", lint.stdout);
    assert_eq!(unused[0]["rule"], "F401", "{}", lint.stdout);
    let message = unused[0]["message"].as_str().unwrap_or("");
    assert!(message.contains("cell 2"), "{message}");
    let saved = findings(&lint, "notebook_outputs", "beta/outputs.ipynb");
    assert_eq!(saved.len(), 1, "{}", lint.stdout);
    let message = saved[0]["message"].as_str().unwrap_or("");
    assert!(message.contains("cell 2"), "{message}");
    assert!(findings(&lint, "ruff", "beta/outputs.ipynb").is_empty());
    assert!(findings(&lint, "notebook_outputs", "alpha/dirty.ipynb").is_empty());
    assert_eq!(read(&dirty), before_dirty);

    let format = dx(&root, "format", &["--output=json"]);
    assert_eq!(format.status.code(), Some(1), "{}", format.combined());
    let changes = of_kind(&events(&format), "change");
    assert_eq!(changes.len(), 1, "{}", format.stdout);
    assert_eq!(changes[0]["path"], "alpha/dirty.ipynb");
    assert_eq!(changes[0]["effect"], "proposed");
    assert_eq!(read(&dirty), before_dirty);

    let formatted = dx(&root, "format", &["--apply", "--output=json"]);
    assert_eq!(formatted.status.code(), Some(0), "{}", formatted.combined());
    let linted = dx(&root, "lint", &["--apply", "--output=json"]);
    assert_eq!(linted.status.code(), Some(0), "{}", linted.combined());
    let applied = of_kind(&events(&linted), "change");
    assert_eq!(applied.len(), 2, "{}", linted.stdout);

    let after = read(&dirty);
    assert_eq!(
        masked(&after),
        masked(&before_dirty),
        "bytes outside cell sources changed"
    );
    let (old, new) = (sources(&before_dirty), sources(&after));
    assert_eq!(old[0], new[0]);
    assert!(!new[1].contains("import os"), "{new:?}");
    assert_eq!(new[2], WRAPPED, "{new:?}");
    let stripped = read(&outputs);
    assert!(before_outputs.contains("\"stream\""));
    assert!(!stripped.contains("\"stream\""), "{stripped}");
    assert_eq!(sources(&stripped), sources(&before_outputs));
    for (path, text) in clean.iter().zip(&before_clean) {
        assert_eq!(&read(path), text);
    }

    for command in ["lint", "format"] {
        let run = dx(&root, command, &["--output=json"]);
        assert_eq!(run.status.code(), Some(0), "{command}: {}", run.combined());
    }
}
