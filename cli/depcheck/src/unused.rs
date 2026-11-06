use std::collections::BTreeSet;
use std::path::Path;

use crate::exceptions::normalize_exception_key;
use crate::{DepcheckError, Ecosystem};

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

fn kept(text: &str, line: usize) -> bool {
    line > 0
        && text
            .lines()
            .nth(line - 1)
            .and_then(|l| l.split_once('#'))
            .is_some_and(|(_, comment)| comment.trim_start().starts_with("dx-keep"))
}

/// Declared dependencies of `manifest` without JS peers and `# dx-keep` lines.
pub fn declared(eco: Ecosystem, manifest: &Path) -> Result<Vec<DeclaredDep>, DepcheckError> {
    let mut deps = crate::consistency::load_manifest(eco, manifest)?;
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let mut out = Vec::new();
    for info in deps.values() {
        let line = declaration_line(&text, &info.raw);
        if kept(&text, line) {
            continue;
        }
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

/// Declared dependencies of `manifest` that nothing under `sources` uses, minus `excepted`.
pub fn find_unused(
    eco: Ecosystem,
    manifest: &Path,
    sources: &Path,
    excepted: &[String],
) -> Result<Vec<UnusedDep>, DepcheckError> {
    let mut deps = crate::consistency::load_manifest(eco, manifest)?;
    if matches!(eco, Ecosystem::Js | Ecosystem::Ts) {
        deps.retain(|_, v| !v.peer);
    }
    let excepted: BTreeSet<String> = excepted
        .iter()
        .map(|name| normalize_exception_key(eco, name))
        .collect();
    let names: Vec<String> = deps.keys().cloned().collect();
    let usages = crate::usage::find_usages(eco, sources, &names)?;
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let mut out = Vec::new();
    for (key, info) in &deps {
        let usage = usages.get(key).copied().unwrap_or_default();
        if usage.src || usage.test || usage.build || excepted.contains(key) {
            continue;
        }
        let line = declaration_line(&text, &info.raw);
        if kept(&text, line) {
            continue;
        }
        out.push(UnusedDep {
            name: info.raw.clone(),
            line,
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
        let found = find_unused(Ecosystem::Rust, &manifest, dir.path(), &[]).expect("find unused");
        assert_eq!(
            found,
            vec![UnusedDep {
                name: "regex".into(),
                line: 6
            }]
        );
        let found = find_unused(Ecosystem::Rust, &manifest, dir.path(), &["regex".into()])
            .expect("find unused");
        assert!(found.is_empty());
    }

    #[test]
    fn declared_uses_the_lock_name_and_skips_dx_keep() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nre = { package = \"regex\", version = \"1\" }\ncc = \"1\" # dx-keep\n",
        );
        let manifest = dir.path().join("Cargo.toml");
        let got = declared(Ecosystem::Rust, &manifest).expect("declared");
        let hubs: Vec<(&str, &str)> = got
            .iter()
            .map(|d| (d.name.as_str(), d.hub.as_str()))
            .collect();
        assert_eq!(hubs, vec![("re", "regex"), ("serde", "serde")]);
        let found = find_unused(Ecosystem::Rust, &manifest, dir.path(), &[]).expect("unused");
        assert!(found.iter().all(|d| d.name != "cc"));
    }
}
