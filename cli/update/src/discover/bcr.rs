//! Lists `bazel_dep` modules whose declared version is behind the newest stable BCR version.

use dx_error::DxError;

use super::{Candidate, Fetch, Findings, LookupError, wanted};
use dx_module_bazel::{ModuleFile, Value, compare_module_versions};
use std::cmp::Ordering;

pub const DEFAULT_BASE: &str = "https://bcr.bazel.build";

const SKIPPED_OVERRIDES: [&str; 3] = ["local_path_override", "git_override", "archive_override"];

/// Checks each `bazel_dep` of `module` against the registry at `base`.
pub fn outdated(
    set: &str,
    module: &ModuleFile,
    only: &[String],
    prereleases: bool,
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
        let body = fetch.get(&url);
        match body.and_then(|body| Ok((newest(&body, prereleases)?, repo(&body)))) {
            Ok((latest, repo)) => {
                if compare_module_versions(&current, &latest) == Ordering::Less {
                    let prerelease = latest.contains('-');
                    found.candidates.push(Candidate {
                        set: set.to_owned(),
                        package: name.to_owned(),
                        current,
                        latest,
                        published: None,
                        prerelease,
                        repo,
                        tags: None,
                    });
                }
            }
            Err(reason) => found.errors.push(error(reason.to_string())),
        }
    }
    found
}

/// Returns the newest stable version of `name` in the registry at `base`.
pub fn latest_stable(name: &str, base: &str, fetch: &dyn Fetch) -> Result<String, DxError> {
    newest(
        &fetch.get(&format!("{base}/modules/{name}/metadata.json"))?,
        false,
    )
}

/// The text of a newest-version snapshot: a JSON object from each `bazel_dep` of `module` that no
/// path, git or archive override replaces to its newest stable version in the registry at `base`.
pub fn newest_snapshot(
    module: &ModuleFile,
    base: &str,
    fetch: &dyn Fetch,
) -> Result<String, DxError> {
    let mut newest = std::collections::BTreeMap::new();
    for call in module.calls_named("bazel_dep") {
        let Some(Value::Str(name)) = call.arg("name") else {
            continue;
        };
        let name = name.value.as_str();
        if is_skipped(module, name) {
            continue;
        }
        let version = latest_stable(name, base, fetch)
            .map_err(|error| DxError::failure(format!("{name}: {error}")))?;
        newest.insert(name.to_owned(), version);
    }
    let text =
        serde_json::to_string_pretty(&newest).map_err(|e| DxError::failure(e.to_string()))?;
    Ok(text + "\n")
}

fn repo(body: &str) -> Option<String> {
    let metadata: serde_json::Value = serde_json::from_str(body).ok()?;
    metadata
        .get("repository")?
        .as_array()?
        .iter()
        .filter_map(|entry| entry.as_str())
        .find_map(super::github_repo)
}

fn is_skipped(module: &ModuleFile, name: &str) -> bool {
    SKIPPED_OVERRIDES.iter().any(|kind| {
        module.calls_named(kind).any(|call| {
            matches!(call.arg("module_name"), Some(Value::Str(literal)) if literal.value == name)
        })
    })
}

fn newest(body: &str, prereleases: bool) -> Result<String, DxError> {
    let metadata: serde_json::Value =
        serde_json::from_str(body).map_err(|e| DxError::failure(e.to_string()))?;
    let versions = metadata
        .get("versions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| DxError::failure("metadata has no versions"))?;
    let yanked = metadata.get("yanked_versions");
    versions
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|v| (prereleases || !v.contains('-')) && yanked.and_then(|y| y.get(*v)).is_none())
        .max_by(|a, b| compare_module_versions(a, b))
        .map(str::to_owned)
        .ok_or_else(|| DxError::failure("no stable version"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = r#"{"versions":["1.0.0","1.2.0","2.0.0-rc1"],"yanked_versions":{}}"#;

    #[test]
    fn metadata_names_the_github_repository() {
        let body = r#"{"repository":["https://gitlab.com/x/y","github:o/rules_x"],"versions":[]}"#;
        assert_eq!(repo(body).as_deref(), Some("o/rules_x"));
        assert_eq!(repo(METADATA), None);
    }

    fn module(text: &str) -> ModuleFile {
        ModuleFile::parse(text).expect("parse")
    }

    fn run(text: &str, body: &'static str) -> Findings {
        outdated(
            "bazel",
            &module(text),
            &[],
            false,
            DEFAULT_BASE,
            &|_: &str| Ok(body.to_owned()),
        )
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let found = outdated(
            "bazel",
            &module("bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n"),
            &[],
            true,
            DEFAULT_BASE,
            &|_: &str| Ok(METADATA.to_owned()),
        );
        assert_eq!(found.candidates[0].latest, "2.0.0-rc1");
        assert!(found.candidates[0].prerelease);
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
                prerelease: false,
                repo: None,
                tags: None,
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
    fn the_newest_snapshot_lists_each_registry_module_once() {
        let text = "bazel_dep(name = \"rules_b\", version = \"1.0.0\")\n\
            bazel_dep(name = \"rules_a\", version = \"2.0.0\", dev_dependency = True)\n\
            bazel_dep(name = \"patched\", version = \"1.0.0\")\n\
            archive_override(module_name = \"patched\", urls = [\"https://example.com/p.tar.gz\"])\n";
        let snapshot = newest_snapshot(&module(text), DEFAULT_BASE, &|url: &str| {
            assert!(!url.contains("patched"), "{url}");
            Ok(METADATA.to_owned())
        });
        assert_eq!(
            snapshot.expect("snapshot"),
            "{\n  \"rules_a\": \"1.2.0\",\n  \"rules_b\": \"1.2.0\"\n}\n"
        );
        let failed = newest_snapshot(&module(text), DEFAULT_BASE, &|_: &str| {
            Err(DxError::failure("HTTP 404"))
        });
        assert!(
            failed
                .expect_err("lookup failure")
                .to_string()
                .contains("rules_b: HTTP 404"),
            "names the module"
        );
    }

    #[test]
    fn lookup_failures_are_reported_per_module() {
        let found = outdated(
            "bazel",
            &module("bazel_dep(name = \"rules_x\", version = \"1.0.0\")\n"),
            &[],
            false,
            DEFAULT_BASE,
            &|_: &str| Err(DxError::failure("HTTP 404")),
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
            false,
            DEFAULT_BASE,
            &|_: &str| Err(DxError::failure("must not fetch")),
        );
        assert_eq!(found, Findings::default());
    }
}
