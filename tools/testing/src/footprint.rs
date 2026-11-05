//! Measures what a consumer workspace pulls in and checks it against a committed budget.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{json, Value};

const COUNTS: [&str; 3] = ["modules", "lock_extensions", "external_repos"];
const LOCK_HEADROOM_PERCENT: u64 = 5;

/// The size of a consumer after a command ran in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footprint {
    pub modules: u64,
    pub lock_bytes: u64,
    pub lock_extensions: u64,
    pub external_repos: u64,
}

impl Footprint {
    pub fn to_json(&self) -> Value {
        json!({
            "modules": self.modules,
            "lock_bytes": self.lock_bytes,
            "lock_extensions": self.lock_extensions,
            "external_repos": self.external_repos,
        })
    }

    fn count(&self, name: &str) -> u64 {
        match name {
            "modules" => self.modules,
            "lock_extensions" => self.lock_extensions,
            _ => self.external_repos,
        }
    }
}

/// Measures a consumer from its `bazel mod graph --output=json` result, its `MODULE.bazel.lock`
/// text and the `external` directory of its output base.
pub fn measure(
    mod_graph_json: &str,
    lock_text: &str,
    external_dir: &Path,
) -> Result<Footprint, String> {
    Ok(Footprint {
        modules: count_modules(mod_graph_json)?,
        lock_bytes: lock_text.len() as u64,
        lock_extensions: count_extensions(lock_text)?,
        external_repos: count_repos(external_dir)?,
    })
}

/// Measures only the lock: its size and its module extension count.
pub fn measure_lock(lock_text: &str) -> Result<Footprint, String> {
    Ok(Footprint {
        modules: 0,
        lock_bytes: lock_text.len() as u64,
        lock_extensions: count_extensions(lock_text)?,
        external_repos: 0,
    })
}

/// Checks `lock_bytes` and `lock_extensions` of a lock measurement against the budget, with the
/// same rules as `check_budget`.
pub fn check_lock_budget(measured: &Footprint, budget: &Value) -> Result<(), String> {
    let mut lock_only = budget.clone();
    for name in ["modules", "external_repos"] {
        lock_only[name] = json!(0);
    }
    check_budget(measured, &lock_only)
}

fn count_modules(graph: &str) -> Result<u64, String> {
    fn walk(node: &Value, found: &mut BTreeSet<String>) {
        let name = node.get("name").and_then(Value::as_str).unwrap_or_default();
        let version = node
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !version.is_empty() && name != "consumer" && name != "rules_dx" {
            found.insert(name.to_owned());
        }
        for child in node
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            walk(child, found);
        }
    }
    let root: Value = serde_json::from_str(graph)
        .map_err(|error| format!("mod graph output is not JSON: {error}"))?;
    let mut found = BTreeSet::new();
    walk(&root, &mut found);
    Ok(found.len() as u64)
}

fn count_extensions(lock_text: &str) -> Result<u64, String> {
    let lock: Value = serde_json::from_str(lock_text)
        .map_err(|error| format!("MODULE.bazel.lock is not JSON: {error}"))?;
    let extensions = lock.get("moduleExtensions").and_then(Value::as_object);
    Ok(extensions.map_or(0, |extensions| extensions.len() as u64))
}

fn count_repos(external_dir: &Path) -> Result<u64, String> {
    let entries = std::fs::read_dir(external_dir)
        .map_err(|error| format!("read {}: {error}", external_dir.display()))?;
    let mut repos = 0;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read {}: {error}", external_dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let marker = name.starts_with('@') || name.ends_with('@');
        if !marker && entry.path().is_dir() {
            repos += 1;
        }
    }
    Ok(repos)
}

/// Fails when a count is above the budget, or below it (so the budget gets lowered). An unset
/// (`null`) budget always fails. `lock_bytes` is a ceiling with 5% headroom and never fails low.
pub fn check_budget(measured: &Footprint, budget: &Value) -> Result<(), String> {
    let report = |problem: String| format!("{problem}\nmeasured: {}", measured.to_json());
    let mut problems = Vec::new();
    for name in COUNTS {
        let Some(limit) = budget.get(name).and_then(Value::as_u64) else {
            problems.push(format!("{name}: no budget is set"));
            continue;
        };
        let value = measured.count(name);
        if value > limit {
            problems.push(format!("{name}: {value} is above the budget {limit}"));
        } else if value < limit {
            problems.push(format!(
                "{name}: {value} is below the budget, lower the budget to {value}"
            ));
        }
    }
    match budget.get("lock_bytes").and_then(Value::as_u64) {
        None => problems.push("lock_bytes: no budget is set".to_owned()),
        Some(limit) => {
            let ceiling = limit + limit * LOCK_HEADROOM_PERCENT / 100;
            if measured.lock_bytes > ceiling {
                problems.push(format!(
                    "lock_bytes: {} is above the budget {limit} plus {LOCK_HEADROOM_PERCENT}%",
                    measured.lock_bytes
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(report(problems.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn footprint() -> Footprint {
        Footprint {
            modules: 10,
            lock_bytes: 1000,
            lock_extensions: 4,
            external_repos: 20,
        }
    }

    fn budget() -> Value {
        json!({"modules": 10, "lock_bytes": 1000, "lock_extensions": 4, "external_repos": 20})
    }

    #[test]
    fn counts_modules_once_and_skips_the_roots() {
        let graph = r#"{"name":"consumer","version":"","dependencies":[
            {"name":"rules_dx","version":"0.0.0","dependencies":[
                {"name":"a","version":"1","dependencies":[{"name":"b","version":"2"}]}]},
            {"name":"b","version":"2"},{"name":"bazel_tools","version":""}]}"#;
        assert_eq!(count_modules(graph), Ok(2));
    }

    #[test]
    fn counts_extensions_in_the_lock() {
        let lock =
            r#"{"lockFileVersion":1,"moduleExtensions":{"//a:b.bzl%x":{},"//c:d.bzl%y":{}}}"#;
        assert_eq!(count_extensions(lock), Ok(2));
        assert_eq!(count_extensions("{}"), Ok(0));
    }

    #[test]
    fn measures_the_lock_alone() {
        let lock = r#"{"moduleExtensions":{"//a:b.bzl%x":{}}}"#;
        let measured = measure_lock(lock).expect("lock");
        assert_eq!(measured.lock_bytes, lock.len() as u64);
        assert_eq!(measured.lock_extensions, 1);
        let budget = json!({"lock_bytes": lock.len(), "lock_extensions": 1});
        assert_eq!(check_lock_budget(&measured, &budget), Ok(()));
        let low = json!({"lock_bytes": lock.len(), "lock_extensions": 2});
        let error = check_lock_budget(&measured, &low).expect_err("below");
        assert!(error.contains("lower the budget to 1"), "{error}");
    }

    #[test]
    fn counts_repo_directories_and_skips_markers() {
        let dir = tempfile::tempdir().expect("tempdir");
        for repo in ["a+", "b+", "@foo@"] {
            std::fs::create_dir(dir.path().join(repo)).expect("mkdir");
        }
        std::fs::write(dir.path().join("a+.marker"), "").expect("write");
        std::fs::write(dir.path().join("@b+"), "").expect("write");
        assert_eq!(count_repos(dir.path()), Ok(2));
    }

    #[test]
    fn the_budget_passes_when_it_matches() {
        assert_eq!(check_budget(&footprint(), &budget()), Ok(()));
    }

    #[test]
    fn a_count_above_the_budget_fails_with_the_measurement() {
        let mut measured = footprint();
        measured.modules = 11;
        let error = check_budget(&measured, &budget()).expect_err("above");
        assert!(
            error.contains("modules: 11 is above the budget 10"),
            "{error}"
        );
        assert!(error.contains("\"external_repos\":20"), "{error}");
    }

    #[test]
    fn a_count_below_the_budget_asks_to_lower_it() {
        let mut measured = footprint();
        measured.external_repos = 18;
        let error = check_budget(&measured, &budget()).expect_err("below");
        assert!(error.contains("lower the budget to 18"), "{error}");
    }

    #[test]
    fn lock_bytes_has_headroom_and_no_floor() {
        let mut measured = footprint();
        measured.lock_bytes = 1050;
        assert_eq!(check_budget(&measured, &budget()), Ok(()));
        measured.lock_bytes = 10;
        assert_eq!(check_budget(&measured, &budget()), Ok(()));
        measured.lock_bytes = 1051;
        assert!(check_budget(&measured, &budget()).is_err());
    }

    #[test]
    fn an_unset_budget_never_passes() {
        let unset = json!({"modules": null, "lock_bytes": null,
            "lock_extensions": null, "external_repos": null});
        let error = check_budget(&footprint(), &unset).expect_err("unset");
        assert!(error.contains("no budget is set"), "{error}");
        assert!(error.contains("\"modules\":10"), "{error}");
    }
}
