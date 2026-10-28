//! Offline gate every openapi_typescript_client runs before Redocly generates anything.

use std::collections::BTreeMap;
use std::process::ExitCode;

use api_spec_preflight::{check_root, openapi_root_problem};

/// The parsed command line.
struct Args {
    root: String,
    docs: Vec<(String, String)>,
    stamp: String,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut root = None;
    let mut docs = Vec::new();
    let mut stamp = None;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let value = iter.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--root" => root = Some(value.clone()),
            "--stamp" => stamp = Some(value.clone()),
            "--doc" => {
                let (short, path) = value
                    .split_once('=')
                    .ok_or_else(|| format!("--doc {value} is not WORKSPACE_PATH=FILE"))?;
                docs.push((short.to_owned(), path.to_owned()));
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        root: root.ok_or("--root is required")?,
        docs,
        stamp: stamp.ok_or("--stamp is required")?,
    })
}

/// The `path:line:col rule message` lines for one root and its staged documents.
fn problems(root: &str, docs: &BTreeMap<String, String>) -> Vec<String> {
    let Some(text) = docs.get(root) else {
        return vec![format!(
            "{root}:1:1 dx/api-spec-unresolved-ref the root is not among the staged documents"
        )];
    };
    let mut found = Vec::new();
    if let Some(problem) = openapi_root_problem(text) {
        found.push(problem);
    }
    found.extend(check_root(root, docs));
    let mut lines: Vec<String> = found
        .into_iter()
        .map(|p| format!("{root}:{}:{} {} {}", p.line, p.column, p.rule, p.message))
        .collect();
    lines.dedup();
    lines
}

fn run(args: &[String]) -> Result<Vec<String>, String> {
    let parsed = parse(args)?;
    let mut docs = BTreeMap::new();
    for (short, path) in &parsed.docs {
        let text = std::fs::read_to_string(path).map_err(|err| format!("read {path}: {err}"))?;
        docs.insert(short.clone(), text);
    }
    let lines = problems(&parsed.root, &docs);
    if lines.is_empty() {
        std::fs::write(&parsed.stamp, "ok\n")
            .map_err(|err| format!("write {}: {err}", parsed.stamp))?;
    }
    Ok(lines)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(lines) if lines.is_empty() => ExitCode::SUCCESS,
        Ok(lines) => {
            for line in lines {
                eprintln!("{line}");
            }
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("openapi codegen preflight: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(files: &[(&str, &str)]) -> BTreeMap<String, String> {
        files
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn rules(lines: &[String]) -> Vec<&str> {
        lines
            .iter()
            .filter_map(|line| line.split(' ').nth(1))
            .collect()
    }

    #[test]
    fn clean_root_with_fragment_passes() {
        let d = docs(&[
            (
                "a/root.yaml",
                "openapi: 3.1.0\ncomponents:\n  schemas:\n    P:\n      $ref: refs/p.yaml\n",
            ),
            ("a/refs/p.yaml", "type: object\n"),
        ]);
        assert!(problems("a/root.yaml", &d).is_empty());
    }

    #[test]
    fn asyncapi_and_swagger_roots_fail() {
        for text in ["asyncapi: 3.0.0\n", "swagger: '2.0'\n"] {
            let d = docs(&[("r.yaml", text)]);
            assert_eq!(rules(&problems("r.yaml", &d)), ["dx/api-spec-root"]);
        }
    }

    #[test]
    fn remote_and_unresolved_refs_fail() {
        let remote = docs(&[(
            "r.yaml",
            "openapi: 3.1.0\nx:\n  $ref: https://e.com/a.yaml\n",
        )]);
        assert_eq!(
            rules(&problems("r.yaml", &remote)),
            ["dx/api-spec-remote-ref"]
        );
        let missing = docs(&[("r.yaml", "openapi: 3.1.0\nx:\n  $ref: gone.yaml\n")]);
        assert_eq!(
            rules(&problems("r.yaml", &missing)),
            ["dx/api-spec-unresolved-ref"]
        );
    }

    #[test]
    fn syntax_error_fails_once() {
        let d = docs(&[("r.yaml", "openapi: [\n")]);
        assert_eq!(rules(&problems("r.yaml", &d)), ["dx/api-spec-syntax"]);
    }

    #[test]
    fn parse_rejects_bad_flags() {
        let bad = |args: &[&str]| {
            parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>()).is_err()
        };
        assert!(bad(&["--root", "r"]));
        assert!(bad(&["--bogus", "x"]));
        assert!(bad(&["--root", "r", "--stamp", "s", "--doc", "nope"]));
    }
}
