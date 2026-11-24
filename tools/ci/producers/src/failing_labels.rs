use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use crate::test_log_tails::PASSING_OUTCOMES;

const USAGE: &str =
    "usage: producers_bin --failing-labels --out FILE [--dx FILE] [--previous FILE]";

const FAILING_SEVERITIES: [&str; 2] = ["warning", "error"];

#[derive(Debug, Default, PartialEq, Eq)]
struct Run {
    failing: BTreeSet<String>,
    covered: Vec<String>,
    errors: BTreeSet<String>,
    exit_code: Option<i64>,
}

pub fn write(args: &[String]) -> Result<(), String> {
    let (pairs, []) = args.as_chunks::<2>() else {
        return Err(USAGE.to_owned());
    };
    let (mut out, mut dx, mut previous) = (None, None, None);
    for [flag, value] in pairs {
        let slot = match flag.as_str() {
            "--out" => &mut out,
            "--dx" => &mut dx,
            "--previous" => &mut previous,
            _ => return Err(USAGE.to_owned()),
        };
        *slot = Some(Path::new(value));
    }
    let out = out.ok_or_else(|| USAGE.to_owned())?;
    let run = match dx.filter(|path| path.exists()) {
        Some(path) => read_run(
            &std::fs::read_to_string(path)
                .map_err(|error| format!("read {}: {error}", path.display()))?,
        ),
        None => Run::default(),
    };
    let previous = match previous {
        Some(path) => read_labels(
            &std::fs::read_to_string(path)
                .map_err(|error| format!("read {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("{}: {error}", path.display()))?,
        None => BTreeSet::new(),
    };
    let labels = merge(&previous, &run);
    let carried = labels.difference(&failing_keys(&run)).count();
    let text = format!("{}\n", json!({ "labels": labels }));
    dx_atomic_fs::write_scratch(out, text.as_bytes())
        .map_err(|error| format!("write {}: {error}", out.display()))?;
    println!(
        "failing-labels: {} in {} ({carried} carried from the last main baseline)",
        labels.len(),
        out.display()
    );
    Ok(())
}

fn read_labels(text: &str) -> Result<BTreeSet<String>, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    value["labels"]
        .as_array()
        .ok_or("no labels array")?
        .iter()
        .map(|label| {
            label
                .as_str()
                .map(str::to_owned)
                .ok_or("a label is not a string")
        })
        .collect::<Result<_, _>>()
        .map_err(str::to_owned)
}

fn read_run(text: &str) -> Run {
    let mut run = Run::default();
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event["event"].as_str() {
            Some("command_started") => {
                let affected = &event["affected"];
                let widened = items(&affected["widened"]).map(|entry| &entry["scope"]);
                run.covered.extend(
                    items(&affected["targets"])
                        .chain(items(&affected["tests"]))
                        .chain(widened)
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
            }
            Some("test_outcome") => {
                let outcome = event["outcome"].as_str().unwrap_or("unknown");
                if event["final"].as_bool() != Some(false) && !PASSING_OUTCOMES.contains(&outcome) {
                    run.failing.extend(event["target"].as_str().map(label));
                }
            }
            Some("build_failure") => {
                run.failing.extend(
                    event["target"]
                        .as_str()
                        .filter(|target| *target != "unknown")
                        .map(label),
                );
                run.failing.extend(
                    items(&event["targets"])
                        .filter_map(Value::as_str)
                        .map(label),
                );
            }
            Some("diagnostic")
                if FAILING_SEVERITIES.contains(&event["severity"].as_str().unwrap_or_default()) =>
            {
                let origins: Vec<String> = items(&event["origins"])
                    .filter_map(|origin| origin["target"].as_str())
                    .map(label)
                    .collect();
                if origins.is_empty() {
                    run.failing
                        .extend(event["path"].as_str().map(str::to_owned));
                }
                run.failing.extend(origins);
            }
            Some("change") if event["effect"].as_str() == Some("proposed") => {
                run.failing
                    .extend(event["path"].as_str().map(str::to_owned));
            }
            Some("error") => {
                run.errors.extend(event["code"].as_str().map(str::to_owned));
            }
            Some("command_finished") => run.exit_code = event["exit_code"].as_i64(),
            _ => {}
        }
    }
    run
}

fn items(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn failing_keys(run: &Run) -> BTreeSet<String> {
    if !run.failing.is_empty() || run.exit_code == Some(0) {
        return run.failing.clone();
    }
    let why = match run.exit_code {
        None => "no result".to_owned(),
        Some(_) if !run.errors.is_empty() => {
            run.errors.iter().cloned().collect::<Vec<_>>().join(", ")
        }
        Some(code) => format!("exit {code}"),
    };
    BTreeSet::from([format!("dx: {why}")])
}

fn merge(previous: &BTreeSet<String>, run: &Run) -> BTreeSet<String> {
    previous
        .iter()
        .filter(|key| key.starts_with("//") && !run.covered.iter().any(|p| covers(p, key)))
        .cloned()
        .chain(failing_keys(run))
        .collect()
}

fn label(target: &str) -> String {
    target
        .strip_prefix("@@")
        .or_else(|| target.strip_prefix('@'))
        .filter(|rest| rest.starts_with("//"))
        .unwrap_or(target)
        .to_owned()
}

fn covers(pattern: &str, target: &str) -> bool {
    let pattern = label(pattern);
    if pattern == target {
        return true;
    }
    let package = target
        .split_once(':')
        .map_or(target, |(package, _)| package);
    let (pattern, all) = [":all-targets", ":all", ":*"]
        .iter()
        .find_map(|suffix| pattern.strip_suffix(suffix))
        .map_or((pattern.as_str(), false), |stripped| (stripped, true));
    match pattern.strip_suffix("...") {
        Some(root) => package == root.trim_end_matches('/') || package.starts_with(root),
        None => all && package == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::{Run, covers, failing_keys, merge, read_labels, read_run};
    use std::collections::BTreeSet;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn reads_each_kind_of_failure_as_a_label() {
        let run = read_run(
            &[
                r#"{"event":"command_started","affected":{"base":"abc","targets":["//a:lib","//b/..."],"tests":["//a:test"],"widened":[{"scope":"//w/...","reason":"x"}]}}"#,
                r#"{"event":"test_outcome","target":"@@//a:test","outcome":"failed"}"#,
                r#"{"event":"test_outcome","target":"//a:old","outcome":"failed","final":false}"#,
                r#"{"event":"test_outcome","target":"//a:ok","outcome":"passed_after_retry"}"#,
                r#"{"event":"build_failure","target":"//b:bin","count":2,"targets":["//b:bin","//b:other"]}"#,
                r#"{"event":"build_failure","target":"unknown"}"#,
                r#"{"event":"diagnostic","severity":"error","path":"a/x.rs","origins":[{"target":"//a:lib"}]}"#,
                r#"{"event":"diagnostic","severity":"info","origins":[{"target":"//a:quiet"}]}"#,
                r#"{"event":"diagnostic","severity":"warning","path":"loose.md"}"#,
                r#"{"event":"change","path":"a/fmt.rs","effect":"proposed"}"#,
                r#"{"event":"error","code":"incomplete_results","message":"m"}"#,
                "not json",
                r#"{"event":"command_finished","exit_code":1}"#,
            ]
            .join("\n"),
        );
        assert_eq!(
            run.failing,
            set(&[
                "//a:test",
                "//b:bin",
                "//b:other",
                "//a:lib",
                "loose.md",
                "a/fmt.rs"
            ])
        );
        assert_eq!(run.covered, ["//a:lib", "//b/...", "//a:test", "//w/..."]);
        assert_eq!(run.exit_code, Some(1));
    }

    #[test]
    fn a_failed_run_without_labels_gets_one_dx_key() {
        let run = |errors: &[&str], exit_code| Run {
            errors: set(errors),
            exit_code,
            ..Run::default()
        };
        assert_eq!(failing_keys(&run(&[], Some(0))), set(&[]));
        assert_eq!(failing_keys(&run(&[], None)), set(&["dx: no result"]));
        assert_eq!(failing_keys(&run(&[], Some(3))), set(&["dx: exit 3"]));
        assert_eq!(failing_keys(&run(&["b", "a"], Some(1))), set(&["dx: a, b"]));
    }

    #[test]
    fn keeps_earlier_labels_the_run_did_not_reach() {
        let previous = set(&["//a:red", "//b/c:red", "//z:red", "a/fmt.rs", "dx: exit 1"]);
        let run = Run {
            failing: set(&["//a:new"]),
            covered: vec!["//a:red".to_owned(), "//b/...".to_owned()],
            exit_code: Some(1),
            ..Run::default()
        };
        assert_eq!(merge(&previous, &run), set(&["//a:new", "//z:red"]));
        let whole = Run {
            covered: vec!["//...".to_owned()],
            exit_code: Some(0),
            ..Run::default()
        };
        assert_eq!(merge(&previous, &whole), set(&[]));
    }

    #[test]
    fn patterns_cover_their_packages() {
        assert!(covers("//...", "//:root"));
        assert!(covers("//...", "//a/b:c"));
        assert!(covers("//a/...", "//a:c"));
        assert!(covers("//a/...", "//a/b:c"));
        assert!(!covers("//a/...", "//ab:c"));
        assert!(covers("//a/...:all", "//a/b:c"));
        assert!(covers("//a:all", "//a:c"));
        assert!(covers("//a:*", "//a:c"));
        assert!(!covers("//a:all", "//a/b:c"));
        assert!(covers("@@//a:c", "//a:c"));
        assert!(!covers("//a:c", "//a:d"));
        assert!(!covers("//...", "@other//a:c"));
    }

    #[test]
    fn reads_only_a_labels_array_of_strings() {
        assert_eq!(
            read_labels(r#"{"labels":["//a:b"]}"#).expect("labels"),
            set(&["//a:b"])
        );
        assert!(read_labels(r#"{"labels":[1]}"#).is_err());
        assert!(read_labels("{}").is_err());
    }
}
