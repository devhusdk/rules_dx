#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};
use support_matrix as _;
use yaml_serde as _;

use dx_atomic_fs::path_io::{self, PathIoError};
use serde_json::Value;

const DIR: &str = "tools/docs/support_matrix";
const USAGE: &str = "usage: row_tests <bazel> <platform> [bazel flags]";

#[derive(Debug)]
enum RowTestsError {
    Usage,
    NoWorkspace,
    Spawn {
        program: String,
        source: io::Error,
    },
    Bazel {
        verb: String,
        stderr: String,
    },
    QueryLine {
        line: String,
        source: serde_json::Error,
    },
    NoRule {
        line: String,
    },
    Serialize {
        path: String,
        source: serde_json::Error,
    },
    Write(PathIoError),
}

impl fmt::Display for RowTestsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => f.write_str(USAGE),
            Self::NoWorkspace => {
                f.write_str("run with bazel run so BUILD_WORKSPACE_DIRECTORY is set")
            }
            Self::Spawn { program, source } => write!(f, "run {program}: {source}"),
            Self::Bazel { verb, stderr } => write!(f, "bazel {verb} failed: {stderr}"),
            Self::QueryLine { line, source } => write!(f, "bad query line {line}: {source}"),
            Self::NoRule { line } => write!(f, "no rule in {line}"),
            Self::Serialize { path, source } => write!(f, "serialize {path}: {source}"),
            Self::Write(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for RowTestsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            Self::QueryLine { source, .. } | Self::Serialize { source, .. } => Some(source),
            Self::Write(source) => Some(source),
            Self::Usage | Self::NoWorkspace | Self::Bazel { .. } | Self::NoRule { .. } => None,
        }
    }
}

fn bazel(
    program: &str,
    workspace: &Path,
    verb: &str,
    rest: &[&str],
    flags: &[String],
) -> Result<String, RowTestsError> {
    let done = Command::new(program)
        .current_dir(workspace)
        .arg(verb)
        .args(flags)
        .args(rest)
        .output()
        .map_err(|source| RowTestsError::Spawn {
            program: program.to_owned(),
            source,
        })?;
    if !done.status.success() {
        return Err(RowTestsError::Bazel {
            verb: verb.to_owned(),
            stderr: String::from_utf8_lossy(&done.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

fn rules(text: &str) -> Result<Vec<Value>, RowTestsError> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: Value =
                serde_json::from_str(line).map_err(|source| RowTestsError::QueryLine {
                    line: line.to_owned(),
                    source,
                })?;
            value
                .get("rule")
                .cloned()
                .ok_or_else(|| RowTestsError::NoRule {
                    line: line.to_owned(),
                })
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

fn write_json(
    workspace: &Path,
    path: &str,
    value: &impl serde::Serialize,
) -> Result<(), RowTestsError> {
    let mut text =
        serde_json::to_string_pretty(value).map_err(|source| RowTestsError::Serialize {
            path: path.to_owned(),
            source,
        })?;
    text.push('\n');
    path_io::write_scratch(&workspace.join(path), text.as_bytes()).map_err(RowTestsError::Write)
}

fn run() -> Result<(), RowTestsError> {
    let mut args = std::env::args().skip(1);
    let program = args.next().ok_or(RowTestsError::Usage)?;
    let platform = args.next().ok_or(RowTestsError::Usage)?;
    let flags: Vec<String> = args.collect();
    let workspace =
        std::env::var_os("BUILD_WORKSPACE_DIRECTORY").ok_or(RowTestsError::NoWorkspace)?;
    let workspace = Path::new(&workspace);
    let tagged = rules(&bazel(
        &program,
        workspace,
        "query",
        &["attr(tags, \"row:\", //...)", "--output=streamed_jsonproto"],
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
        &program,
        workspace,
        "cquery",
        &[&platforms, &query, "--output=starlark", &starlark],
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
            &program,
            workspace,
            "query",
            &[&query, "--output=streamed_jsonproto"],
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
