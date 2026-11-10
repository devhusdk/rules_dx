use std::collections::BTreeSet;
use std::path::Path;

use crate::DepcheckError;
use crate::dep_key::normalize_dep_key;
use dx_ecosystems::Language;

/// A declared dependency that no source file under the scanned root uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnusedDep {
    pub name: String,
    /// One-based line of the declaration in the manifest, 0 when it cannot be found.
    pub line: usize,
}

/// A dependency declared in a manifest, as the Bazel graph would name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredDep {
    pub name: String,
    /// The lock name when the entry is renamed, else the declared name.
    pub hub: String,
    /// One-based line of the declaration in the manifest, 0 when it cannot be found.
    pub line: usize,
}

fn declaration_line(text: &str, raw: &str) -> usize {
    text.lines()
        .position(|line| line.contains(raw))
        .map_or(0, |at| at + 1)
}

fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Names the manifest itself marks as intentionally unused: cargo-machete `ignored` for Cargo and
/// deptry `DEP002` for pyproject.
fn native_ignores(eco: Language, manifest: &Path) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return BTreeSet::new();
    };
    let Ok(data) = toml::from_str::<toml::Value>(&text) else {
        return BTreeSet::new();
    };
    let names = match eco {
        Language::Rust => {
            let mut names = string_list(
                data.get("package")
                    .and_then(|v| v.get("metadata"))
                    .and_then(|v| v.get("cargo-machete"))
                    .and_then(|v| v.get("ignored")),
            );
            names.extend(string_list(
                data.get("workspace")
                    .and_then(|v| v.get("metadata"))
                    .and_then(|v| v.get("cargo-machete"))
                    .and_then(|v| v.get("ignored")),
            ));
            names
        }
        Language::Python => string_list(
            data.get("tool")
                .and_then(|v| v.get("deptry"))
                .and_then(|v| v.get("per_rule_ignores"))
                .and_then(|v| v.get("DEP002")),
        ),
        _ => Vec::new(),
    };
    names
        .iter()
        .map(|name| normalize_dep_key(eco, name))
        .collect()
}

/// Declared dependencies of `manifest` without JS peers and natively ignored entries.
pub fn declared(eco: Language, manifest: &Path) -> Result<Vec<DeclaredDep>, DepcheckError> {
    let mut deps = crate::consistency::load_manifest(eco, manifest)?;
    if matches!(eco, Language::Js | Language::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let ignored = native_ignores(eco, manifest);
    let mut out = Vec::new();
    for (key, info) in &deps {
        if ignored.contains(key) {
            continue;
        }
        let line = declaration_line(&text, &info.raw);
        let hub = if info.package.is_empty() {
            info.raw.clone()
        } else {
            info.package.clone()
        };
        out.push(DeclaredDep {
            name: info.raw.clone(),
            hub,
            line,
        });
    }
    Ok(out)
}

/// Declared dependencies of `manifest` that nothing under `sources` uses and no target depends on (`used`).
pub fn find_unused(
    eco: Language,
    manifest: &Path,
    sources: &Path,
    used: &[String],
) -> Result<Vec<UnusedDep>, DepcheckError> {
    let mut deps = crate::consistency::load_manifest(eco, manifest)?;
    if matches!(eco, Language::Js | Language::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let mut used: BTreeSet<String> = used
        .iter()
        .map(|name| normalize_dep_key(eco, name))
        .collect();
    used.extend(native_ignores(eco, manifest));
    let names: Vec<String> = deps.keys().cloned().collect();
    let usages = crate::usage::find_usages(eco, sources, &names)?;
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let mut out = Vec::new();
    for (key, info) in &deps {
        let usage = usages.get(key).copied().unwrap_or_default();
        if usage.src || usage.test || usage.build || used.contains(key) {
            continue;
        }
        out.push(UnusedDep {
            name: info.raw.clone(),
            line: declaration_line(&text, &info.raw),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    #[test]
    fn unused_crate_is_reported_with_its_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nregex = \"1\"\n",
        );
        write(dir.path(), "src/lib.rs", "use serde::Serialize;\n");
        let manifest = dir.path().join("Cargo.toml");
        let found = find_unused(Language::Rust, &manifest, dir.path(), &[]).expect("find unused");
        assert_eq!(
            found,
            vec![UnusedDep {
                name: "regex".into(),
                line: 6
            }]
        );
        let found = find_unused(Language::Rust, &manifest, dir.path(), &["regex".into()])
            .expect("find unused");
        assert!(found.is_empty());
    }

    #[test]
    fn declared_uses_the_lock_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nre = { package = \"regex\", version = \"1\" }\n",
        );
        let manifest = dir.path().join("Cargo.toml");
        let got = declared(Language::Rust, &manifest).expect("declared");
        let hubs: Vec<(&str, &str)> = got
            .iter()
            .map(|d| (d.name.as_str(), d.hub.as_str()))
            .collect();
        assert_eq!(hubs, vec![("re", "regex"), ("serde", "serde")]);
    }

    #[test]
    fn cargo_machete_ignored_is_not_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"x\"\n\n[package.metadata.cargo-machete]\nignored = [\"cc\"]\n\n[dependencies]\nserde = \"1\"\ncc = \"1\"\n",
        );
        let manifest = dir.path().join("Cargo.toml");
        let found = find_unused(Language::Rust, &manifest, dir.path(), &[]).expect("unused");
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["serde"]);
        let got = declared(Language::Rust, &manifest).expect("declared");
        assert!(got.iter().all(|d| d.name != "cc"));
    }

    #[test]
    fn deptry_dep002_ignore_is_not_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "pyproject.toml",
            "[project]\nname = \"x\"\nversion = \"1\"\ndependencies = [\"Requests>=2\", \"attrs>=1\"]\n\n[tool.deptry.per_rule_ignores]\nDEP002 = [\"requests\"]\n",
        );
        let manifest = dir.path().join("pyproject.toml");
        let found = find_unused(Language::Python, &manifest, dir.path(), &[]).expect("unused");
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["attrs"]);
    }
}
