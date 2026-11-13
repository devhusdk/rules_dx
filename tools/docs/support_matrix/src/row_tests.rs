//! Writes `row_labels.json`, `consumer_labels.json` and `row_tests/<platform>.json` from Bazel queries.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, ExitCode};

use serde_json::Value;

const DIR: &str = "tools/docs/support_matrix";

fn bazel(workspace: &Path, args: &[&str], flags: &[String]) -> Result<String, String> {
    let (verb, rest) = args.split_first().ok_or("no bazel verb")?;
    let done = Command::new("bazel")
        .current_dir(workspace)
        .arg(verb)
        .args(flags)
        .args(rest)
        .output()
        .map_err(|error| format!("run bazel: {error}"))?;
    if !done.status.success() {
        return Err(format!(
            "bazel {verb} failed: {}",
            String::from_utf8_lossy(&done.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

fn rules(text: &str) -> Result<Vec<Value>, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
            value
                .get("rule")
                .cloned()
                .ok_or_else(|| format!("no rule in {line}"))
        })
        .collect()
}

fn attribute<'a>(rule: &'a Value, name: &str) -> Option<&'a Value> {
    rule.get("attribute")?
        .as_array()?
        .iter()
        .find(|attribute| attribute.get("name").and_then(Value::as_str) == Some(name))
}

fn strings(rule: &Value, name: &str) -> Vec<String> {
    attribute(rule, name)
        .and_then(|attribute| attribute.get("stringListValue"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn label_of(rule: &Value) -> String {
    rule.get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn labels_by_row(rules: &[Value], prefix: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for rule in rules {
        let label = label_of(rule);
        let short = label.rsplit(':').next().unwrap_or_default();
        let generator = attribute(rule, "generator_name")
            .and_then(|attribute| attribute.get("stringValue"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(short);
        if generator != short {
            continue;
        }
        for tag in strings(rule, "tags") {
            if let Some(row) = tag.strip_prefix(prefix) {
                out.entry(row.to_owned()).or_default().insert(label.clone());
            }
        }
    }
    out
}

fn write_json(workspace: &Path, path: &str, value: &impl serde::Serialize) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    std::fs::write(workspace.join(path), text).map_err(|error| format!("write {path}: {error}"))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let platform = args
        .next()
        .ok_or("usage: row_tests <platform> [bazel flags]")?;
    let flags: Vec<String> = args.collect();
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .ok_or("run with bazel run so BUILD_WORKSPACE_DIRECTORY is set")?;
    let workspace = Path::new(&workspace);
    let tagged = rules(&bazel(
        workspace,
        &[
            "query",
            "attr(tags, \"row:\", //...)",
            "--output=streamed_jsonproto",
        ],
        &flags,
    )?)?;
    let mut all = BTreeSet::new();
    for (prefix, file) in [
        ("row:", "row_labels.json"),
        ("consumer-row:", "consumer_labels.json"),
    ] {
        let by_row = labels_by_row(&tagged, prefix);
        all.extend(by_row.values().flatten().cloned());
        write_json(workspace, &format!("{DIR}/{file}"), &by_row)?;
    }
    let query = format!("set({})", all.into_iter().collect::<Vec<_>>().join(" "));
    let expr = format!(
        "str(target.label).removeprefix('@@') if 'IncompatiblePlatformProvider' not in providers(target) and [str(p).removeprefix('@@') for p in build_options(target)['//command_line_option:platforms']] == ['//tools/platforms:{platform}'] else ''"
    );
    let platforms = format!("--platforms=//tools/platforms:{platform}");
    let starlark = format!("--starlark:expr={expr}");
    let compatible = bazel(
        workspace,
        &["cquery", &platforms, &query, "--output=starlark", &starlark],
        &flags,
    )?;
    let compatible: Vec<&str> = compatible
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut tests: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if !compatible.is_empty() {
        let query = format!("set({})", compatible.join(" "));
        let found = rules(&bazel(
            workspace,
            &["query", &query, "--output=streamed_jsonproto"],
            &flags,
        )?)?;
        for rule in &found {
            let tags = strings(rule, "tags")
                .into_iter()
                .filter(|tag| !tag.starts_with("row:") && !tag.starts_with("consumer-row:"));
            tests.entry(label_of(rule)).or_default().extend(tags);
        }
    }
    write_json(
        workspace,
        &format!("{DIR}/row_tests/{platform}.json"),
        &tests,
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
