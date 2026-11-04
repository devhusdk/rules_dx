//! Lists `bazel_dep` modules whose declared version is behind the newest stable BCR version.

use super::{wanted, Candidate, Fetch, Findings, LookupError};
use dx_module_bazel::{compare_module_versions, ModuleFile, Value};
use std::cmp::Ordering;

pub const DEFAULT_BASE: &str = "https://bcr.bazel.build";

const SKIPPED_OVERRIDES: [&str; 3] = ["local_path_override", "git_override", "archive_override"];

/// Checks each `bazel_dep` of `module` against the registry at `base`.
pub fn outdated(
    set: &str,
    module: &ModuleFile,
    only: &[String],
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    for call in module.calls_named("bazel_dep") {
        let Some(Value::Str(name)) = call.arg("name") else {
            continue;
        };
        let name = name.value.as_str();
        if !wanted(only, name) || is_skipped(module, name) {
            continue;
        }
        let error = |reason: String| LookupError {
            set: set.to_owned(),
            package: name.to_owned(),
            reason,
        };
        let current = match module
            .override_version(name)
            .or_else(|_| module.bazel_dep_version(name))
        {
            Ok(version) => version.value.clone(),
            Err(reason) => {
                found.errors.push(error(reason.to_string()));
                continue;
            }
        };
        let url = format!("{base}/modules/{name}/metadata.json");
        match fetch.get(&url).and_then(|body| newest_stable(&body)) {
            Ok(latest) => {
                if compare_module_versions(&current, &latest) == Ordering::Less {
                    found.candidates.push(Candidate {
                        set: set.to_owned(),
                        package: name.to_owned(),
                        current,
                        latest,
                        published: None,
                    });
                }
            }
            Err(reason) => found.errors.push(error(reason)),
        }
    }
    found
}

fn is_skipped(module: &ModuleFile, name: &str) -> bool {
    SKIPPED_OVERRIDES.iter().any(|kind| {
        module.calls_named(kind).any(|call| {
            matches!(call.arg("module_name"), Some(Value::Str(literal)) if literal.value == name)
        })
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = r#"{"versions":["1.0.0","1.2.0","2.0.0-rc1"],"yanked_versions":{}}"#;

    fn module(text: &str) -> ModuleFile {
        ModuleFile::parse(text).expect("parse")
    }

    fn run(text: &str, body: &'static str) -> Findings {
        outdated("bazel", &module(text), &[], DEFAULT_BASE, &|_: &str| {
            Ok(body.to_owned())
        })
    }

    #[test]
    fn behind_the_newest_stable_is_outdated() {
        let found = run(
            "bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n",
            METADATA,
        );
        assert_eq!(
            found.candidates,
            vec![Candidate {
                set: "bazel".into(),
                package: "rules_x".into(),
                current: "1.0.0".into(),
                latest: "1.2.0".into(),
                published: None,
            }]
        );
        assert!(found.errors.is_empty());
    }

    #[test]
    fn current_modules_are_not_listed() {
        let found = run(
            "bazel_dep(name = \"rules_x\", version = \"1.2.0\")\n",
            METADATA,
        );
        assert_eq!(found, Findings::default());
    }

    #[test]
    fn yanked_versions_are_skipped() {
        let body = r#"{"versions":["1.0.0","1.2.0"],"yanked_versions":{"1.2.0":"bad"}}"#;
        let found = run("bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n", body);
        assert_eq!(found, Findings::default());
    }

    #[test]
    fn path_overrides_are_skipped() {
        let text = "bazel_dep(name = \"rules_x\", version = \"1.0.0\")\nlocal_path_override(module_name = \"rules_x\", path = \"x\")\n";
        assert_eq!(run(text, METADATA), Findings::default());
    }

    #[test]
    fn lookup_failures_are_reported_per_module() {
        let found = outdated(
            "bazel",
            &module("bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n"),
            &[],
            DEFAULT_BASE,
            &|_: &str| Err("HTTP 404".to_owned()),
        );
        assert!(found.candidates.is_empty());
        assert_eq!(found.errors[0].package, "rules_x");
        assert_eq!(found.errors[0].reason, "HTTP 404");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = outdated(
            "bazel",
            &module("bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n"),
            &["other".to_owned()],
            DEFAULT_BASE,
            &|_: &str| Err("must not fetch".to_owned()),
        );
        assert_eq!(found, Findings::default());
    }
}
