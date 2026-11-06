use super::update_unused::{self, Missing, Via};
use dx_adopt::dependency_sets::{Ecosystem, ResolvedTarget};
use dx_plan::resolve::QueryRunner;
use dx_update::discover::{Fetch, cargo, npm, pypi, ruby};
use dx_update::edit::{AddError, AddRequest, ConfiguredKind, UpdateRequest};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug)]
pub(super) struct Added {
    pub set: String,
    pub name: String,
    pub version: String,
    pub manifest: String,
}

fn kind_of(ecosystem: Ecosystem) -> Option<ConfiguredKind> {
    match ecosystem {
        Ecosystem::Cargo => Some(ConfiguredKind::Cargo),
        Ecosystem::Npm => Some(ConfiguredKind::Npm),
        Ecosystem::Uv => Some(ConfiguredKind::Uv),
        Ecosystem::Ruby => Some(ConfiguredKind::Ruby),
        _ => None,
    }
}

fn latest(ecosystem: Ecosystem, name: &str, fetch: &dyn Fetch) -> Result<String, String> {
    match ecosystem {
        Ecosystem::Cargo => cargo::latest_stable(name, cargo::DEFAULT_BASE, fetch),
        Ecosystem::Npm => npm::latest_stable(name, npm::DEFAULT_BASE, fetch),
        Ecosystem::Uv => pypi::latest_stable(name, pypi::DEFAULT_BASE, fetch),
        _ => ruby::latest_stable(name, ruby::DEFAULT_BASE, fetch),
    }
}

/// The manifest of the set whose directory holds `file`, else the first one.
fn manifest_for<'a>(manifests: &'a [String], file: &str) -> Option<&'a String> {
    manifests
        .iter()
        .filter(|manifest| {
            let dir = manifest.rsplit_once('/').map_or("", |(dir, _)| dir);
            dir.is_empty() || file == dir || file.starts_with(&format!("{dir}/"))
        })
        .max_by_key(|manifest| manifest.len())
        .or_else(|| manifests.first())
}

fn plan(
    target: &ResolvedTarget,
    item: &Missing,
    kind: ConfiguredKind,
    texts: &mut BTreeMap<String, String>,
    workspace: &Path,
    fetch: &dyn Fetch,
) -> Result<Option<Added>, String> {
    let set = &target.set;
    let near = match &item.via {
        Via::Import { file, .. } => file.as_str(),
        Via::Label(label) => label
            .trim_start_matches('/')
            .split(':')
            .next()
            .unwrap_or_default(),
    };
    let manifest = manifest_for(&set.manifests, near)
        .ok_or_else(|| "the set has no manifest".to_owned())?
        .clone();
    let version = latest(set.ecosystem, &item.name, fetch)?;
    let update = UpdateRequest::parse_configured_set(
        kind,
        &set.name,
        std::slice::from_ref(&manifest),
        set.repository.as_deref(),
        &format!("{}:{}", set.name, item.name),
        &version,
    )
    .map_err(|error| error.to_string())?;
    let request = AddRequest::from_request(&update);
    if !texts.contains_key(&manifest) {
        let text = std::fs::read_to_string(workspace.join(&manifest))
            .map_err(|error| format!("{manifest}: {error}"))?;
        texts.insert(manifest.clone(), text);
    }
    let inserted = match request.plan_insert(&texts[&manifest]) {
        Ok(inserted) => inserted,
        Err(AddError::Declared { .. }) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    texts.insert(manifest.clone(), inserted);
    Ok(Some(Added {
        set: set.name.clone(),
        name: item.name.clone(),
        version,
        manifest,
    }))
}

/// Declares every imported but undeclared dependency of the resolved sets at its latest stable
/// version. Every manifest is written before any lock is refreshed.
pub(super) fn add_missing(
    workspace: &Path,
    resolved: &[ResolvedTarget],
    fetch: &dyn Fetch,
    query: &dyn QueryRunner,
    startup: &[String],
) -> Result<Vec<Added>, String> {
    let mut texts = BTreeMap::new();
    let mut added = Vec::new();
    let mut failures = Vec::new();
    for item in update_unused::find_missing(workspace, resolved, query, startup) {
        let Some(target) = resolved.iter().find(|target| target.set.name == item.set) else {
            continue;
        };
        let Some(kind) = kind_of(target.set.ecosystem) else {
            continue;
        };
        match plan(target, &item, kind, &mut texts, workspace, fetch) {
            Ok(Some(done)) => added.push(done),
            Ok(None) => {}
            Err(reason) => failures.push(format!("{}:{} ({reason})", item.set, item.name)),
        }
    }
    for manifest in added.iter().map(|done| &done.manifest) {
        std::fs::write(workspace.join(manifest), &texts[manifest])
            .map_err(|error| format!("{manifest}: {error}"))?;
    }
    if failures.is_empty() {
        Ok(added)
    } else {
        Err(format!(
            "failed to add missing dependencies: {}",
            failures.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_adopt::dependency_sets::{DependencySet, ResolvedSet, Selection};

    struct NoQuery;

    impl QueryRunner for NoQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &Path,
        ) -> std::io::Result<dx_plan::resolve::QueryResult> {
            Err(std::io::Error::other("no bazel"))
        }
    }

    fn target(set: &str, ecosystem: Ecosystem, manifest: &str, lock: &str) -> ResolvedTarget {
        ResolvedTarget {
            set: ResolvedSet::of(&DependencySet {
                name: set.to_owned(),
                ecosystem,
                manifests: vec![manifest.to_owned()],
                locks: vec![lock.to_owned()],
                scopes: Vec::new(),
                repository: None,
            }),
            request: Selection::Full,
        }
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn registry(url: &str) -> Result<String, String> {
        if url.ends_with("/re/ge/regex") {
            Ok("{\"vers\":\"1.5.0\",\"yanked\":false}\n{\"vers\":\"2.0.0-rc.1\",\"yanked\":false}\n".to_owned())
        } else if url.ends_with("/left-pad") {
            Ok("{\"dist-tags\":{\"latest\":\"1.3.0\"}}".to_owned())
        } else {
            Err(format!("unexpected {url}"))
        }
    }

    #[test]
    fn a_package_directory_matches_the_manifest_beside_its_build_file() {
        let manifests = vec!["a/Cargo.toml".to_owned(), "b/Cargo.toml".to_owned()];
        assert_eq!(manifest_for(&manifests, "b").unwrap(), "b/Cargo.toml");
        assert_eq!(manifest_for(&manifests, "b/src/lib.rs").unwrap(), "b/Cargo.toml");
        assert_eq!(manifest_for(&manifests, "ab").unwrap(), "a/Cargo.toml");
    }

    #[test]
    fn declares_the_latest_stable_version_of_each_missing_package() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "rs/Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\n",
        );
        write(
            root,
            "rs/src/lib.rs",
            "use serde::Serialize;\nuse regex::Regex;\n",
        );
        write(
            root,
            "rs/Cargo.lock",
            "[[package]]\nname = \"regex\"\nversion = \"1.5.0\"\nsource = \"registry+https://x\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://x\"\n",
        );
        write(
            root,
            "web/package.json",
            "{\n  \"dependencies\": {\n    \"react\": \"18.0.0\"\n  }\n}\n",
        );
        write(
            root,
            "web/index.js",
            "import pad from 'left-pad';\nimport React from 'react';\n",
        );
        write(
            root,
            "web/pnpm-lock.yaml",
            "lockfileVersion: '9.0'\n\npackages:\n\n  left-pad@1.3.0:\n    resolution: {integrity: sha512-x}\n\n  react@18.0.0:\n    resolution: {integrity: sha512-y}\n",
        );
        let resolved = [
            target("crates", Ecosystem::Cargo, "rs/Cargo.toml", "rs/Cargo.lock"),
            target(
                "npm",
                Ecosystem::Npm,
                "web/package.json",
                "web/pnpm-lock.yaml",
            ),
        ];
        let added = add_missing(root, &resolved, &registry, &NoQuery, &[]).expect("added");
        let names: Vec<_> = added
            .iter()
            .map(|a| format!("{} {} {}", a.set, a.name, a.version))
            .collect();
        assert_eq!(names, ["crates regex 1.5.0", "npm left-pad 1.3.0"]);
        let cargo = std::fs::read_to_string(root.join("rs/Cargo.toml")).unwrap();
        assert!(cargo.contains("regex = \"1.5.0\""), "{cargo}");
        let npm = std::fs::read_to_string(root.join("web/package.json")).unwrap();
        assert!(npm.contains("\"left-pad\": \"1.3.0\""), "{npm}");
        assert!(update_unused::find_missing(root, &resolved, &NoQuery, &[]).is_empty());
        assert!(
            add_missing(root, &resolved, &registry, &NoQuery, &[])
                .expect("again")
                .is_empty()
        );
    }

    #[test]
    fn a_failed_lookup_is_reported_and_the_others_are_still_written() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "web/package.json",
            "{\n  \"dependencies\": {\n    \"react\": \"18.0.0\"\n  }\n}\n",
        );
        write(
            root,
            "web/index.js",
            "import pad from 'left-pad';\nimport z from 'zzz';\n",
        );
        write(
            root,
            "web/pnpm-lock.yaml",
            "lockfileVersion: '9.0'\n\npackages:\n\n  left-pad@1.3.0:\n    resolution: {integrity: sha512-x}\n\n  zzz@1.0.0:\n    resolution: {integrity: sha512-y}\n",
        );
        let resolved = [target(
            "npm",
            Ecosystem::Npm,
            "web/package.json",
            "web/pnpm-lock.yaml",
        )];
        let error = add_missing(root, &resolved, &registry, &NoQuery, &[]).expect_err("zzz fails");
        assert!(error.contains("npm:zzz"), "{error}");
        let npm = std::fs::read_to_string(root.join("web/package.json")).unwrap();
        assert!(npm.contains("\"left-pad\": \"1.3.0\""), "{npm}");
    }
}
