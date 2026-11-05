use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::DepcheckError;

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
        for (line, import) in go_imports(&String::from_utf8_lossy(&bytes))? {
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
}
