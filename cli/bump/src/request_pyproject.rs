//! Widening one PEP 508 requirement in a `pyproject.toml`.

use super::BumpError;
use crate::version::WidenVersion;
use toml_edit::{Array, DocumentMut, Item, Value};

const MANIFEST: &str = "pyproject.toml";

fn unsupported(reason: impl Into<String>) -> BumpError {
    BumpError::UnsupportedManifest {
        manifest: MANIFEST.to_owned(),
        reason: reason.into(),
    }
}

/// PEP 503 normalization: lowercase with runs of `-`, `_` and `.` as one `-`.
fn normalize(name: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            separator = true;
        } else {
            if separator && !out.is_empty() {
                out.push('-');
            }
            separator = false;
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

/// The requirement widened to `>=version`, or `None` when it names another package.
fn widen_requirement(
    requirement: &str,
    package: &str,
    version: &str,
) -> Result<Option<String>, BumpError> {
    let trimmed = requirement.trim_start();
    let name_end = trimmed
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(trimmed.len());
    let (name, rest) = trimmed.split_at(name_end);
    if normalize(name) != normalize(package) {
        return Ok(None);
    }
    let lead = &requirement[..requirement.len() - trimmed.len()];
    let after_name = rest.trim_start();
    let (extras, after_extras) = match after_name.strip_prefix('[') {
        Some(inner) => {
            let close = inner
                .find(']')
                .ok_or_else(|| unsupported(format!("unterminated extras in {requirement:?}")))?;
            (&after_name[..close + 2], inner[close + 1..].trim_start())
        }
        None => ("", after_name),
    };
    if after_extras.starts_with('@') || after_extras.starts_with('(') {
        return Err(unsupported(format!(
            "{package:?} requirement is a direct reference or parenthesized"
        )));
    }
    let (_, marker) = match after_extras.find(';') {
        Some(at) => after_extras.split_at(at),
        None => (after_extras, ""),
    };
    let marker = if marker.is_empty() {
        String::new()
    } else {
        format!(" {}", marker.trim_end())
    };
    Ok(Some(format!("{lead}{name}{extras}>={version}{marker}")))
}

/// Widens every requirement array in `array`, collecting `(index, widened)` hits.
fn scan_array(
    array: &Array,
    package: &str,
    version: &str,
    hits: &mut Vec<String>,
) -> Result<Vec<(usize, String)>, BumpError> {
    let mut found = Vec::new();
    for (index, value) in array.iter().enumerate() {
        if let Some(text) = value.as_str() {
            if let Some(widened) = widen_requirement(text, package, version)? {
                hits.push(widened.clone());
                found.push((index, widened));
            }
        }
    }
    Ok(found)
}

pub(crate) fn plan_pyproject_toml(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let WidenVersion::Semver(version) = version else {
        return Err(unsupported("expected exact semver"));
    };
    let version = version.to_string();
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|_| unsupported("manifest is not valid TOML"))?;
    // Each entry is the path of a requirement array.
    let mut paths: Vec<Vec<String>> = vec![
        vec!["project".into(), "dependencies".into()],
        vec!["tool".into(), "uv".into(), "dev-dependencies".into()],
    ];
    for table in ["optional-dependencies"] {
        if let Some(group) = doc
            .get("project")
            .and_then(|item| item.get(table))
            .and_then(Item::as_table_like)
        {
            for (key, _) in group.iter() {
                paths.push(vec!["project".into(), table.into(), key.to_owned()]);
            }
        }
    }
    if let Some(group) = doc.get("dependency-groups").and_then(Item::as_table_like) {
        for (key, _) in group.iter() {
            paths.push(vec!["dependency-groups".into(), key.to_owned()]);
        }
    }
    let mut hits = Vec::new();
    let mut edits: Vec<(Vec<String>, Vec<(usize, String)>)> = Vec::new();
    for path in paths {
        let mut item = Some(doc.as_item());
        for key in &path {
            item = item.and_then(|item| item.get(key.as_str()));
        }
        let Some(array) = item.and_then(Item::as_array) else {
            continue;
        };
        let found = scan_array(array, package, &version, &mut hits)?;
        if !found.is_empty() {
            edits.push((path, found));
        }
    }
    match hits.len() {
        0 => Err(BumpError::NotFound {
            manifest: MANIFEST.to_owned(),
            package: package.to_owned(),
        }),
        1 => {
            let (path, found) = edits.remove(0);
            let mut item = doc.as_item_mut();
            for key in &path {
                item = item
                    .get_mut(key.as_str())
                    .ok_or_else(|| unsupported("requirement array vanished"))?;
            }
            let array = item
                .as_array_mut()
                .ok_or_else(|| unsupported("requirement array vanished"))?;
            for (index, widened) in found {
                let slot = array
                    .get_mut(index)
                    .ok_or_else(|| unsupported("requirement vanished"))?;
                let decor = slot.decor().clone();
                let mut value = Value::from(widened);
                *value.decor_mut() = decor;
                *slot = value;
            }
            Ok(doc.to_string())
        }
        count => Err(BumpError::Ambiguous {
            manifest: MANIFEST.to_owned(),
            package: package.to_owned(),
            count,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widen(content: &str, package: &str, version: &str) -> Result<String, BumpError> {
        let version = WidenVersion::Semver(version.parse().expect("semver"));
        plan_pyproject_toml(content, package, &version)
    }

    const PYPROJECT: &str = r#"[project]
name = "frontend"
dependencies = [
    "anyio>=4",  # io
    "Typing_Extensions[extra] >=4.0, <5 ; python_version < '3.12'",
]

[project.optional-dependencies]
dev = ["pytest==8.0.0"]

[dependency-groups]
lint = ["ruff>=0.1"]
"#;

    #[test]
    fn widens_one_requirement_and_keeps_the_rest_of_the_file() {
        let out = widen(PYPROJECT, "anyio", "4.1.0").expect("widen");
        assert_eq!(out, PYPROJECT.replace("anyio>=4", "anyio>=4.1.0"));
    }

    #[test]
    fn matches_normalized_names_and_keeps_extras_and_markers() {
        let out = widen(PYPROJECT, "typing-extensions", "4.9.0").expect("widen");
        assert!(
            out.contains(r#""Typing_Extensions[extra]>=4.9.0 ; python_version < '3.12'""#),
            "{out}"
        );
    }

    #[test]
    fn widens_optional_and_group_requirements() {
        let out = widen(PYPROJECT, "pytest", "8.1.0").expect("widen");
        assert!(out.contains(r#"dev = ["pytest>=8.1.0"]"#), "{out}");
        let out = widen(PYPROJECT, "ruff", "0.2.0").expect("widen");
        assert!(out.contains(r#"lint = ["ruff>=0.2.0"]"#), "{out}");
    }

    #[test]
    fn missing_and_ambiguous_requirements_fail() {
        assert!(matches!(
            widen(PYPROJECT, "absent", "1.0.0"),
            Err(BumpError::NotFound { .. })
        ));
        let twice = "[project]\ndependencies = [\"a>=1\"]\n[dependency-groups]\ng = [\"a>=2\"]\n";
        assert!(matches!(
            widen(twice, "a", "3.0.0"),
            Err(BumpError::Ambiguous { count: 2, .. })
        ));
    }

    #[test]
    fn direct_references_fail_closed() {
        let text = "[project]\ndependencies = [\"a @ https://example.com/a.whl\"]\n";
        assert!(matches!(
            widen(text, "a", "1.0.0"),
            Err(BumpError::UnsupportedManifest { .. })
        ));
    }
}
