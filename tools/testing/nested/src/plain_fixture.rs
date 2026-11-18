//! Runs every check on a rule-set plain fixture: the consumer shape and the footprint budget.

use std::path::Path;

use serde_json::Value;

use crate::{Case, footprint, plain_consumer};

/// Returns `Ok` when the fixture is a plain consumer of `rules_<lang>` and its measured footprint matches `budget`.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
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
#[must_use]
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
/// # Errors
///
/// Returns `Err` when a Bazel command fails or the measurement cannot be read.
pub fn measure_case(case: &Case) -> Result<footprint::Footprint, String> {
    let graph = run_ok(case, &["mod", "graph", "--output=json"], "mod graph")?;
    let info = run_ok(case, &["info", "output_base"], "info output_base")?;
    let external = std::path::PathBuf::from(info.stdout.trim()).join("external");
    let lock = dx_testing::read_text(&case.root().join("MODULE.bazel.lock"));
    footprint::measure(&graph.stdout, &lock, &external)
}

/// Returns `Ok` when every C++ toolchain selected in `resolution` comes from a `local_config_cc` repository.
///
/// # Errors
///
/// Returns `Err` when no C++ toolchain was selected or one comes from another repository.
pub fn check_own_cc(resolution: &str) -> Result<(), String> {
    let selected: Vec<&str> = resolution
        .lines()
        .filter(|line| line.contains("Selected") && line.contains("tools/cpp:toolchain_type"))
        .collect();
    if selected.is_empty() {
        return Err(format!("no C++ toolchain selected:\n{resolution}"));
    }
    let foreign: Vec<&str> = selected
        .iter()
        .copied()
        .filter(|line| !selected_repo(line).is_some_and(|repo| repo.rsplit('+').next() == Some("local_config_cc")))
        .collect();
    if foreign.is_empty() {
        Ok(())
    } else {
        Err(format!("C++ toolchain is not local_config_cc:\n{}", foreign.join("\n")))
    }
}

fn selected_repo(line: &str) -> Option<&str> {
    let label = line.split("-> toolchain ").nth(1)?.split_whitespace().next()?;
    label.trim_start_matches('@').split("//").next()
}

/// Checks that `target` in the case resolves its C++ toolchain to `local_config_cc`.
///
/// # Errors
///
/// Returns `Err` when the cquery fails or another C++ toolchain is selected.
pub fn assert_own_cc(case: &Case, target: &str) -> Result<(), String> {
    let run = run_ok(
        case,
        &[
            "cquery",
            "--color=no",
            "--curses=no",
            "--toolchain_resolution_debug=@bazel_tools//tools/cpp:toolchain_type",
            target,
        ],
        "cquery toolchain resolution",
    )?;
    check_own_cc(&run.combined()).map_err(|error| format!("{target}: {error}"))
}

fn run_ok(case: &Case, args: &[&str], what: &str) -> Result<dx_testing::Run, String> {
    let run = case.bazel(args);
    if run.status.code() == Some(0) {
        Ok(run)
    } else {
        Err(format!("{what}: {}", run.combined()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const GRAPH: &str = r#"{"name":"consumer","version":"","dependencies":[
        {"name":"rules_lua","version":"0.0.0","dependencies":[{"name":"a","version":"1"}]}]}"#;
    const LOCK: &str = r#"{"moduleExtensions":{"//a:b.bzl%x":{}}}"#;

    fn module(dep: &str) -> String {
        format!("module(name = \"consumer\")\nbazel_dep(name = \"{dep}\", version = \"0.0.0\")\n")
    }

    fn external() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["rules_lua+", "a+"] {
            std::fs::create_dir_all(dir.path().join(name)).expect("repo");
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
        check_fixture("lua", &module(dep), &files, GRAPH, LOCK, dir.path(), budget)
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

    const SELECTED: &str = "INFO: ToolchainResolution: Target platform @@platforms//host:host: Selected execution platform @@platforms//host:host, type @@bazel_tools//tools/cpp:toolchain_type -> toolchain ";

    #[test]
    fn accepts_local_config_cc() {
        let text = format!("{SELECTED}@@rules_cc++cc_configure_extension+local_config_cc//:cc-compiler-k8\n");
        assert_eq!(check_own_cc(&text), Ok(()));
    }

    #[test]
    fn rejects_a_rules_dx_cc_toolchain() {
        let text = format!(
            "{SELECTED}@@rules_cc++cc_configure_extension+local_config_cc//:cc-compiler-k8\n{SELECTED}@@llvm+//toolchain:cc\n"
        );
        let error = check_own_cc(&text).expect_err("llvm");
        assert!(error.contains("@@llvm+//toolchain:cc") && !error.contains("cc-compiler-k8"), "{error}");
    }

    #[test]
    fn rejects_a_lookalike_repo() {
        let text = format!("{SELECTED}@@other+local_config_cc_x//:cc\n");
        assert!(check_own_cc(&text).is_err());
    }

    #[test]
    fn rejects_no_selection() {
        let error = check_own_cc("INFO: nothing\n").expect_err("empty");
        assert!(error.contains("no C++ toolchain"), "{error}");
    }

    #[test]
    fn lists_dotfiles_and_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".dx")).expect("dir");
        std::fs::write(dir.path().join(".dx/bazelrc"), "").expect("file");
        std::fs::write(dir.path().join("BUILD.bazel"), "").expect("file");
        assert_eq!(
            fixture_files(dir.path()),
            [".dx", ".dx/bazelrc", "BUILD.bazel"]
        );
    }
}
