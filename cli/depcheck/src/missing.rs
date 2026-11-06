use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{DepcheckError, Ecosystem};

/// A third-party import that no declared dependency covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDep {
    pub name: String,
    /// Source file of the first import, relative to the scanned root.
    pub file: String,
    pub line: usize,
}

fn go_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                let skipped = matches!(
                    name.as_str(),
                    ".git" | "node_modules" | "vendor" | "testdata"
                ) || name.starts_with("bazel-")
                    || name.starts_with('.')
                    || path.join("go.mod").is_file();
                if !skipped {
                    stack.push(path);
                }
            } else if name.ends_with(".go") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn covers(module: &str, import: &str) -> bool {
    import == module
        || import
            .strip_prefix(module)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn go_imports(text: &str) -> Result<Vec<(usize, String)>, DepcheckError> {
    let quoted =
        regex::Regex::new(r#"^\s*(?:[\w.]+\s+)?"([^"]+)""#).map_err(DepcheckError::SourcesRegex)?;
    let single = regex::Regex::new(r#"^import\s+(?:[\w.]+\s+)?"([^"]+)""#)
        .map_err(DepcheckError::SourcesRegex)?;
    let mut out = Vec::new();
    let mut in_block = false;
    for (at, line) in text.lines().enumerate() {
        if in_block {
            if line.trim_start().starts_with(')') {
                in_block = false;
            } else if let Some(found) = quoted.captures(line) {
                out.push((at + 1, found[1].to_owned()));
            }
        } else if line.starts_with("import") {
            if line.trim_end().ends_with('(') {
                in_block = true;
            } else if let Some(found) = single.captures(line) {
                out.push((at + 1, found[1].to_owned()));
            }
        }
    }
    Ok(out)
}

/// Go imports under `sources` of a module that neither the module itself nor a `require` covers.
pub fn find_missing_go(
    manifest: &Path,
    sources: &Path,
    excepted: &[String],
) -> Result<Vec<MissingDep>, DepcheckError> {
    let text = std::fs::read_to_string(manifest).map_err(DepcheckError::ManifestIo)?;
    let file = dx_gomod::parse_mod(&text).map_err(DepcheckError::ManifestParse)?;
    let mut found: BTreeMap<String, MissingDep> = BTreeMap::new();
    for path in go_files(sources) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(sources)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = String::from_utf8_lossy(&bytes);
        for (line, import) in go_imports(&text)? {
            let first = import.split('/').next().unwrap_or("");
            let declared = !first.contains('.')
                || covers(&file.module, &import)
                || file.require.iter().any(|r| covers(&r.module, &import))
                || excepted.iter().any(|name| covers(name, &import));
            if !declared {
                found.entry(import.clone()).or_insert(MissingDep {
                    name: import,
                    file: rel.clone(),
                    line,
                });
            }
        }
    }
    Ok(found.into_values().collect())
}

fn registry_packages(lock: &Path, eco: Ecosystem) -> Result<Vec<String>, DepcheckError> {
    let text = std::fs::read_to_string(lock).map_err(DepcheckError::LockIo)?;
    let data: toml::Value = toml::from_str(&text).map_err(DepcheckError::LockToml)?;
    let mut out = Vec::new();
    for item in data
        .get("package")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let external = match (eco, item.get("source")) {
            (Ecosystem::Python, Some(source)) => {
                source.get("registry").is_some()
                    || source.get("git").is_some()
                    || source.get("url").is_some()
            }
            (_, source) => source.is_some(),
        };
        if external {
            out.push(name.to_owned());
        }
    }
    Ok(out)
}

fn lock_packages(eco: Ecosystem, lock: &Path) -> Result<Vec<String>, DepcheckError> {
    match eco {
        Ecosystem::Rust => registry_packages(lock, eco),
        Ecosystem::Python => Ok(registry_packages(lock, eco)?
            .iter()
            .map(|name| crate::normalize_py(name))
            .collect()),
        Ecosystem::Js | Ecosystem::Ts => Ok(crate::parse_pnpm_lock(lock)?
            .package_names()
            .into_iter()
            .collect()),
        Ecosystem::Ruby => Ok(crate::parse_paren_lock(lock)?.into_keys().collect()),
        Ecosystem::Perl => Ok(crate::parse_perl_lock(lock)?.into_keys().collect()),
        _ => Ok(Vec::new()),
    }
}

/// Packages in `lock` that a source under `sources` imports but `manifest` does not declare.
/// Workspace and path members are not candidates. A package imported under a different name
/// (`PyYAML` as `yaml`) is not found.
pub fn find_missing(
    eco: Ecosystem,
    manifest: &Path,
    lock: &Path,
    sources: &Path,
    excepted: &[String],
) -> Result<Vec<MissingDep>, DepcheckError> {
    let declared = crate::consistency::load_manifest(eco, manifest)?;
    let key = |name: &str| crate::dep_key::normalize_dep_key(eco, name);
    let excepted: BTreeSet<String> = excepted.iter().map(|name| key(name)).collect();
    let mut candidates: Vec<String> = lock_packages(eco, lock)?
        .into_iter()
        .filter(|name| {
            let name = key(name);
            !declared.contains_key(&name) && !excepted.contains(&name)
        })
        .collect();
    candidates.sort();
    candidates.dedup();
    let nested = match eco {
        Ecosystem::Rust => "Cargo.toml",
        Ecosystem::Python => "pyproject.toml",
        Ecosystem::Ruby => "Gemfile",
        Ecosystem::Perl => "cpanfile",
        _ => "package.json",
    };
    let found = crate::usage::find_first_imports(eco, sources, nested, &candidates)?;
    Ok(found
        .into_iter()
        .map(|(name, (file, line))| MissingDep { name, file, line })
        .collect())
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
    fn go_import_without_a_require_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(
            dir.path(),
            "go.mod",
            "module example.com/app\n\ngo 1.22\n\nrequire github.com/a/b v1.0.0\n",
        );
        write(
            dir.path(),
            "main.go",
            "package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/app/util\"\n\tx \"github.com/a/b/sub\"\n\t\"github.com/c/d\"\n)\n\nimport \"github.com/e/f\"\n",
        );
        write(dir.path(), "nested/go.mod", "module example.com/nested\n");
        write(
            dir.path(),
            "nested/n.go",
            "package n\nimport \"github.com/g/h\"\n",
        );
        let manifest = dir.path().join("go.mod");
        let found = find_missing_go(&manifest, dir.path(), &[]).expect("find missing");
        let got: Vec<(&str, usize)> = found.iter().map(|m| (m.name.as_str(), m.line)).collect();
        assert_eq!(got, vec![("github.com/c/d", 7), ("github.com/e/f", 10)]);
        let found = find_missing_go(&manifest, dir.path(), &["github.com/e/f".into()])
            .expect("find missing");
        assert_eq!(found.len(), 1);
    }

    fn eco_case(eco: Ecosystem, files: &[(&str, &str)], manifest: &str, lock: &str) -> Vec<String> {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        find_missing(
            eco,
            &dir.path().join(manifest),
            &dir.path().join(lock),
            dir.path(),
            &[],
        )
        .expect("find missing")
        .iter()
        .map(|m| format!("{}@{}:{}", m.name, m.file, m.line))
        .collect()
    }

    #[test]
    fn rust_transitive_crate_is_missing_but_local_mod_is_not() {
        let lock = "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\nsource = \"registry+https://x\"\n\n[[package]]\nname = \"itoa\"\nversion = \"1.0.0\"\nsource = \"registry+https://x\"\n\n[[package]]\nname = \"time\"\nversion = \"0.3.0\"\nsource = \"registry+https://x\"\n";
        let got = eco_case(
            Ecosystem::Rust,
            &[
                ("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n"),
                ("Cargo.lock", lock),
                ("src/lib.rs", "mod time;\nuse serde::Serialize;\n\nfn f() {\n    itoa::Buffer::new();\n}\n"),
                ("member/Cargo.toml", "[package]\nname = \"m\"\n"),
                ("member/src/lib.rs", "use time::Duration;\n"),
            ],
            "Cargo.toml",
            "Cargo.lock",
        );
        assert_eq!(got, vec!["itoa@src/lib.rs:5"]);
    }

    #[test]
    fn npm_transitive_package_is_missing() {
        let lock = "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n      react:\n        specifier: ^18\n        version: 18.0.0\npackages:\n  react@18.0.0:\n    resolution: {integrity: x}\n  lodash@4.0.0:\n    resolution: {integrity: x}\nsnapshots:\n  react@18.0.0: {}\n  lodash@4.0.0: {}\n";
        let got = eco_case(
            Ecosystem::Js,
            &[
                ("package.json", "{\"dependencies\": {\"react\": \"^18\"}}"),
                ("pnpm-lock.yaml", lock),
                (
                    "src/a.js",
                    "import React from 'react';\nconst l = require('lodash/fp');\n",
                ),
            ],
            "package.json",
            "pnpm-lock.yaml",
        );
        assert_eq!(got, vec!["lodash@src/a.js:2"]);
    }

    #[test]
    fn uv_transitive_package_is_missing() {
        let lock = "version = 1\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\nsource = { editable = \".\" }\n\n[[package]]\nname = \"requests\"\nversion = \"2.0.0\"\nsource = { registry = \"https://pypi.org/simple\" }\n\n[[package]]\nname = \"urllib3\"\nversion = \"2.0.0\"\nsource = { registry = \"https://pypi.org/simple\" }\n";
        let got = eco_case(
            Ecosystem::Python,
            &[
                ("pyproject.toml", "[project]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"requests>=2\"]\n"),
                ("uv.lock", lock),
                ("app/main.py", "import requests\nimport app\n\nfrom urllib3 import PoolManager\n"),
            ],
            "pyproject.toml",
            "uv.lock",
        );
        assert_eq!(got, vec!["urllib3@app/main.py:4"]);
    }

    #[test]
    fn ruby_transitive_gem_is_missing() {
        let lock = "GEM\n  remote: https://rubygems.org/\n  specs:\n    rack (3.0.0)\n    sinatra (3.0.0)\n      rack (~> 3)\n";
        let got = eco_case(
            Ecosystem::Ruby,
            &[
                ("Gemfile", "source 'https://rubygems.org'\ngem 'sinatra'\n"),
                ("Gemfile.lock", lock),
                ("app.rb", "require 'sinatra'\nrequire 'rack/utils'\n"),
            ],
            "Gemfile",
            "Gemfile.lock",
        );
        assert_eq!(got, vec!["rack@app.rb:2"]);
    }
}
