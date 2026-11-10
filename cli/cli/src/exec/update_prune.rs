use super::update_unused::Unused;
use dx_error::DxError;
use std::collections::BTreeMap;
use std::path::Path;

/// Deletes the one-line declarations of unused dependencies and returns the entries removed.
pub(super) fn prune(workspace: &Path, unused: &[Unused]) -> Result<Vec<Unused>, DxError> {
    let mut by_manifest: BTreeMap<&str, Vec<&Unused>> = BTreeMap::new();
    for item in unused {
        by_manifest.entry(&item.manifest).or_default().push(item);
    }
    let mut removed = Vec::new();
    for (manifest, mut items) in by_manifest {
        let path = workspace.join(manifest);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| DxError::from_io(format!("{manifest}: {e}"), &e))?;
        let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
        items.sort_by_key(|item| std::cmp::Reverse(item.line));
        let json = manifest.ends_with(".json");
        let mut changed = false;
        for item in items {
            if item.line == 0
                || item.line > lines.len()
                || !removable(&lines[item.line - 1], &item.name)
            {
                continue;
            }
            let last = lines.remove(item.line - 1);
            if json && !last.trim_end().ends_with(',') {
                drop_trailing_comma(&mut lines, item.line - 1);
            }
            removed.push(Unused {
                set: item.set.clone(),
                name: item.name.clone(),
                manifest: item.manifest.clone(),
                line: item.line,
            });
            changed = true;
        }
        if changed {
            std::fs::write(&path, lines.concat())
                .map_err(|e| DxError::from_io(format!("{manifest}: {e}"), &e))?;
        }
    }
    removed.sort_by(|a, b| (&a.set, &a.name).cmp(&(&b.set, &b.name)));
    Ok(removed)
}

/// A single-line entry that names the dependency and opens no table or block.
fn removable(line: &str, name: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[\"") {
        return false;
    }
    if trimmed.ends_with(['{', '[', '(']) {
        return false;
    }
    let lower = trimmed.to_lowercase();
    let name = name.to_lowercase();
    lower.contains(&name) || lower.contains(&name.replace('-', "_"))
}

fn drop_trailing_comma(lines: &mut [String], before: usize) {
    let Some(prev) = lines[..before]
        .iter_mut()
        .rev()
        .find(|l| !l.trim().is_empty())
    else {
        return;
    };
    let body = prev.trim_end();
    if let Some(stripped) = body.strip_suffix(',') {
        let ending = &prev[body.len()..];
        *prev = format!("{stripped}{ending}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, manifest: &str, line: usize) -> Unused {
        Unused {
            set: "s".to_owned(),
            name: name.to_owned(),
            manifest: manifest.to_owned(),
            line,
        }
    }

    #[test]
    fn removes_a_crate_and_the_last_npm_entry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[dependencies]\nserde = \"1\"\nregex = \"1\"\n\n[dependencies.big]\nversion = \"1\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("package.json"),
            "{\n  \"dependencies\": {\n    \"react\": \"18.0.0\",\n    \"left-pad\": \"1.0.0\"\n  }\n}\n",
        )
        .unwrap();
        let found = [
            item("regex", "Cargo.toml", 3),
            item("big", "Cargo.toml", 5),
            item("left-pad", "package.json", 4),
        ];
        let removed = prune(root, &found).unwrap();
        assert_eq!(removed.len(), 2);
        assert_eq!(
            std::fs::read_to_string(root.join("Cargo.toml")).unwrap(),
            "[dependencies]\nserde = \"1\"\n\n[dependencies.big]\nversion = \"1\"\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("package.json")).unwrap(),
            "{\n  \"dependencies\": {\n    \"react\": \"18.0.0\"\n  }\n}\n"
        );
    }
}
