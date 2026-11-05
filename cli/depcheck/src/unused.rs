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

/// One `[[exception]]` entry of `depcheck.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetException {
    pub set: String,
    pub name: String,
    pub reason: String,
}

/// Reads the `[[exception]] set, name, reason` entries; a missing file has none.
pub fn parse_set_exceptions(path: &Path) -> Result<Vec<SetException>, DepcheckError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(DepcheckError::ExceptionsIo(error)),
    };
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::ExceptionsToml)?;
    let items = data
        .get("exception")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let field = |item: &toml::Value, key: &str| {
        item.get(key)
            .and_then(crate::toml_util::toml_string)
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    let mut out = Vec::new();
    for item in items {
        let name = field(&item, "name");
        if name.is_empty() {
            return Err(DepcheckError::ExceptionEntry);
        }
        out.push(SetException {
            set: field(&item, "set"),
            name,
            reason: field(&item, "reason"),
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
        let line = text
            .lines()
            .position(|line| line.contains(info.raw.as_str()))
            .map_or(0, |at| at + 1);
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
    fn set_exceptions_read_set_name_and_reason() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(parse_set_exceptions(&dir.path().join("depcheck.toml"))
            .expect("missing")
            .is_empty());
        write(
            dir.path(),
            "depcheck.toml",
            "[[exception]]\nset = \"npm\"\nname = \"left-pad\"\nreason = \"loaded by a plugin\"\n",
        );
        let got = parse_set_exceptions(&dir.path().join("depcheck.toml")).expect("parse");
        assert_eq!(
            got,
            vec![SetException {
                set: "npm".into(),
                name: "left-pad".into(),
                reason: "loaded by a plugin".into()
            }]
        );
    }
}
