//! Lists the `bazel_dep` modules whose declared version is behind the newest stable BCR version.

use dx_module_bazel::{ModuleFile, Value, compare_module_versions};
use std::cmp::Ordering;

const SKIPPED_OVERRIDES: [&str; 3] = ["local_path_override", "git_override", "archive_override"];

/// Returns `bazel:<module> <newest>` lines for outdated deps and `bazel:<module> (BCR: <reason>)` errors.
pub fn outdated(
    module: &ModuleFile,
    fetch: &dyn Fn(&str) -> Result<String, String>,
) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    for call in module.calls_named("bazel_dep") {
        let Some(Value::Str(name)) = call.arg("name") else {
            continue;
        };
        let name = name.value.as_str();
        if is_skipped(module, name) {
            continue;
        }
        let current = match module
            .override_version(name)
            .or_else(|_| module.bazel_dep_version(name))
        {
            Ok(version) => version.value.clone(),
            Err(error) => {
                errors.push(format!("bazel:{name} (BCR: {error})"));
                continue;
            }
        };
        match fetch(name).and_then(|body| newest_stable(&body)) {
            Ok(newest) => {
                if compare_module_versions(&current, &newest) == Ordering::Less {
                    lines.push(format!("bazel:{name} {newest}"));
                }
            }
            Err(reason) => errors.push(format!("bazel:{name} (BCR: {reason})")),
        }
    }
    (lines, errors)
}

/// True when a non-registry override pins the module.
fn is_skipped(module: &ModuleFile, name: &str) -> bool {
    SKIPPED_OVERRIDES.iter().any(|kind| {
        module.calls_named(kind).any(|call| {
            matches!(call.arg("module_name"), Some(Value::Str(literal)) if literal.value == name)
        })
    })
}

/// The newest version in `metadata.json` that is neither yanked nor a prerelease.
fn newest_stable(body: &str) -> Result<String, String> {
    let metadata: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let versions = metadata["versions"]
        .as_array()
        .ok_or("metadata has no versions")?;
    let yanked = &metadata["yanked_versions"];
    versions
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|v| !v.contains('-') && yanked.get(*v).is_none())
        .max_by(|a, b| compare_module_versions(a, b))
        .map(str::to_owned)
        .ok_or_else(|| "no stable version".to_owned())
}
