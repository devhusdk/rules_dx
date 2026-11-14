//! Runs every check on a rule-set plain fixture: the consumer shape and the footprint budget.

use std::path::Path;

use serde_json::Value;

use crate::{Case, footprint, plain_consumer};

/// Returns `Ok` when the fixture is a plain consumer of `rules_<lang>` and its measured footprint matches `budget`.
pub fn check_fixture(
    lang: &str,
    module_bazel: &str,
    files: &[String],
    graph_json: &str,
    lock_text: &str,
    external_dir: &Path,
    budget: &Value,
) -> Result<(), String> {
    let names: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut problems = Vec::new();
    if let Err(error) = plain_consumer::check_plain_consumer(lang, module_bazel, &names) {
        problems.push(error);
    }
    match footprint::measure(graph_json, lock_text, external_dir) {
        Ok(measured) => {
            if let Err(error) = footprint::check_budget(&measured, budget) {
                problems.push(error);
            }
        }
        Err(error) => problems.push(error),
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Returns the paths below `root` relative to it, dotfiles and dot directories included.
pub fn fixture_files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    collect(root, "", &mut found);
    found.sort();
    found
}

fn collect(dir: &Path, prefix: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}{name}");
        if entry.path().is_dir() {
            found.push(relative.clone());
            collect(&entry.path(), &format!("{relative}/"), found);
        } else {
            found.push(relative);
        }
    }
}

/// Measures the footprint of the module the case's nested Bazel resolved.
///
/// Panics when a Bazel command fails or the measurement cannot be read.
pub fn measure_case(case: &Case) -> footprint::Footprint {
    let graph = run_ok(case, &["mod", "graph", "--output=json"], "mod graph");
    let info = run_ok(case, &["info", "output_base"], "info output_base");
    let external = std::path::PathBuf::from(info.stdout.trim()).join("external");
    let lock = dx_testing::read_text(&case.root().join("MODULE.bazel.lock"));
    footprint::measure(&graph.stdout, &lock, &external).unwrap_or_else(|error| panic!("{error}"))
}

fn run_ok(case: &Case, args: &[&str], what: &str) -> dx_testing::Run {
    let run = case.bazel(args);
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GRAPH: &str = r#"{"name":"consumer","version":"","dependencies":[
        {"name":"rules_lua","version":"0.0.0","dependencies":[{"name":"a","version":"1"}]}]}"#;
    const LOCK: &str = r#"{"moduleExtensions":{"//a:b.bzl%x":{}}}"#;

    fn module(dep: &str) -> String {
        format!(
            "module(name = \"consumer\")\nbazel_dep(name = \"{dep}\", version = \"0.0.0\")\n"
        )
    }

    fn external() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["rules_lua+", "a+"] {
            std::fs::create_dir(dir.path().join(name)).expect("repo");
        }
        dir
    }

    fn budget(modules: u64) -> Value {
        let lock_bytes = LOCK.len();
        json!({"modules": modules, "lock_bytes": lock_bytes, "lock_extensions": 1, "external_repos": 2})
    }

    fn check(dep: &str, files: &[&str], budget: &Value) -> Result<(), String> {
        let dir = external();
        let files: Vec<String> = files.iter().map(|file| (*file).to_owned()).collect();
        check_fixture(
            "lua",
            &module(dep),
            &files,
            GRAPH,
            LOCK,
            dir.path(),
            budget,
        )
    }

    #[test]
    fn accepts_a_clean_fixture() {
        assert_eq!(check("rules_lua", &["BUILD.bazel"], &budget(2)), Ok(()));
    }

    #[test]
    fn rejects_the_dx_bazelrc() {
        let error = check("rules_lua", &[".dx/bazelrc"], &budget(2)).expect_err("dx");
        assert!(error.contains(".dx/bazelrc"), "{error}");
    }

    #[test]
    fn rejects_a_rules_dx_dep() {
        let error = check("rules_dx", &[], &budget(2)).expect_err("dep");
        assert!(error.contains("rules_dx"), "{error}");
    }

    #[test]
    fn rejects_an_exceeded_budget() {
        let error = check("rules_lua", &[], &budget(1)).expect_err("budget");
        assert!(error.contains("above the budget"), "{error}");
    }

    #[test]
    fn rejects_a_stale_high_budget() {
        let error = check("rules_lua", &[], &budget(3)).expect_err("budget");
        assert!(error.contains("lower the budget"), "{error}");
    }

    #[test]
    fn lists_dotfiles_and_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join(".dx")).expect("dir");
        std::fs::write(dir.path().join(".dx/bazelrc"), "").expect("file");
        std::fs::write(dir.path().join("BUILD.bazel"), "").expect("file");
        assert_eq!(
            fixture_files(dir.path()),
            [".dx", ".dx/bazelrc", "BUILD.bazel"]
        );
    }
}
