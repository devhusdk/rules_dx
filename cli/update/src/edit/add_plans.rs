//! Insert planners for the manifests a missing `dx update` package is written to. Each one keeps every other byte.

use crate::edit::request::plans::{JsonScan, NPM_SECTIONS, paket_tokens};

fn line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Adds `package = "version"` to `[dependencies]`, creating the table when missing and
/// keeping it sorted when it already is.
pub(super) fn insert_cargo_dependency(
    content: &str,
    package: &str,
    version: &str,
) -> Result<String, String> {
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "manifest is not valid TOML".to_owned())?;
    let item = doc.entry("dependencies").or_insert_with(toml_edit::table);
    let Some(table) = item.as_table_mut() else {
        return Err("[dependencies] is not a table".to_owned());
    };
    if table.contains_key(package) {
        return Err(format!("{package:?} is already in [dependencies]"));
    }
    let keys: Vec<String> = table.iter().map(|(key, _)| key.to_owned()).collect();
    let sorted = keys.windows(2).all(|pair| pair[0] <= pair[1]);
    table.insert(package, toml_edit::value(version));
    if sorted && !keys.is_empty() {
        table.sort_values();
    }
    let out = doc.to_string();
    let reparsed = out
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "edit did not round-trip".to_owned())?;
    let kept = reparsed
        .get("dependencies")
        .and_then(|deps| deps.get(package))
        .and_then(|dep| dep.as_str())
        == Some(version);
    if !kept {
        return Err("edit did not round-trip".to_owned());
    }
    Ok(out)
}

/// Adds `"package": "version"` to `dependencies`, creating the section when missing.
pub(super) fn insert_npm_dependency(
    content: &str,
    package: &str,
    version: &str,
) -> Result<String, String> {
    let bad = |_| "manifest is not valid JSON".to_owned();
    let root: serde_json::Value = serde_json::from_str(content).map_err(bad)?;
    let Some(root_object) = root.as_object() else {
        return Err("manifest root is not an object".to_owned());
    };
    let eol = line_ending(content);
    let key = serde_json::to_string(package).map_err(bad)?;
    let value = serde_json::to_string(version).map_err(bad)?;
    let mut scan = JsonScan {
        text: content,
        at: 0,
    };
    scan.skip_ws();
    let root_open = scan.at;
    let members = scan
        .members()
        .map_err(|_| "manifest is not valid JSON".to_owned())?;
    let root_close = scan.at;
    let Some(&(_, last_root_value)) = members.last() else {
        return Err("manifest root has no members".to_owned());
    };
    let unit = line_indent(content, last_root_value);
    let multiline = content[root_open..root_close].contains('\n');
    let out = match members.iter().find(|(name, _)| name == "dependencies") {
        Some(&(_, at)) => {
            if !content[at..].starts_with('{') {
                return Err("dependency section is not an object".to_owned());
            }
            scan.at = at;
            let entries = scan
                .members()
                .map_err(|_| "manifest is not valid JSON".to_owned())?;
            let close = scan.at;
            match entries.last() {
                Some(&(_, last)) => {
                    scan.at = last;
                    scan.skip_value()
                        .map_err(|_| "manifest is not valid JSON".to_owned())?;
                    let end = scan.at;
                    let entry = if content[at..close].contains('\n') {
                        format!(",{eol}{}{key}: {value}", line_indent(content, last))
                    } else {
                        format!(", {key}: {value}")
                    };
                    splice(content, end, &entry)
                }
                None => {
                    let parent = line_indent(content, at);
                    let entry = if multiline {
                        format!("{{{eol}{parent}{unit}{key}: {value}{eol}{parent}}}")
                    } else {
                        format!("{{ {key}: {value} }}")
                    };
                    format!("{}{entry}{}", &content[..at], &content[close + 1..])
                }
            }
        }
        None => {
            let section = if multiline {
                format!("\"dependencies\": {{{eol}{unit}{unit}{key}: {value}{eol}{unit}}}")
            } else {
                format!("\"dependencies\": {{ {key}: {value} }}")
            };
            let before_dev = members
                .iter()
                .position(|(name, _)| name == "devDependencies")
                .filter(|index| *index > 0);
            let after = match before_dev {
                Some(index) => members[index - 1].1,
                None => last_root_value,
            };
            scan.at = after;
            scan.skip_value()
                .map_err(|_| "manifest is not valid JSON".to_owned())?;
            let end = scan.at;
            let entry = if multiline {
                format!(",{eol}{unit}{section}")
            } else {
                format!(", {section}")
            };
            splice(content, end, &entry)
        }
    };
    let reparsed: serde_json::Value =
        serde_json::from_str(&out).map_err(|_| "edit did not round-trip".to_owned())?;
    let mut expected = root_object.clone();
    let deps = expected
        .entry("dependencies")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(deps) = deps.as_object_mut() {
        deps.insert(
            package.to_owned(),
            serde_json::Value::String(version.to_owned()),
        );
    }
    if reparsed != serde_json::Value::Object(expected) {
        return Err("edit did not round-trip".to_owned());
    }
    let declared = NPM_SECTIONS
        .iter()
        .filter(|section| {
            reparsed
                .get(**section)
                .and_then(|s| s.get(package))
                .is_some()
        })
        .count();
    if declared != 1 {
        return Err("edit did not round-trip".to_owned());
    }
    Ok(out)
}

/// The leading whitespace of the line holding byte `at`.
fn line_indent(content: &str, at: usize) -> &str {
    let start = content[..at].rfind('\n').map_or(0, |index| index + 1);
    let line = &content[start..at];
    &line[..line.len() - line.trim_start().len()]
}

fn splice(content: &str, at: usize, text: &str) -> String {
    let mut out = String::with_capacity(content.len() + text.len());
    out.push_str(&content[..at]);
    out.push_str(text);
    out.push_str(&content[at..]);
    out
}

/// Adds `nuget package version` after the last `nuget` line of the main group.
pub(super) fn insert_paket_dependency(
    content: &str,
    package: &str,
    version: &str,
) -> Result<String, String> {
    let mut anchor = None;
    let mut fallback = None;
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let tokens = paket_tokens(line);
        let Some((_, _, kind)) = tokens.first() else {
            continue;
        };
        if kind.eq_ignore_ascii_case("group") {
            break;
        }
        if kind.eq_ignore_ascii_case("nuget") {
            anchor = Some((start, line));
        } else {
            fallback = Some((start, line));
        }
    }
    let Some((start, line)) = anchor.or(fallback) else {
        return Err("no main-group declaration to add after".to_owned());
    };
    let eol = line_ending(content);
    let indent = &line[..line.len() - line.trim_start().len()];
    let end = start + line.len();
    let mut entry = String::new();
    if !line.ends_with('\n') {
        entry.push_str(eol);
    }
    let indent = if anchor.is_some() { indent } else { "" };
    if anchor.is_none() {
        entry.push_str(eol);
    }
    entry.push_str(&format!("{indent}nuget {package} {version}{eol}"));
    Ok(splice(content, end, &entry))
}

/// Adds `package>=version` to `[project].dependencies`, creating the array when missing.
pub(super) fn insert_pyproject_dependency(
    content: &str,
    package: &str,
    version: &str,
) -> Result<String, String> {
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "manifest is not valid TOML".to_owned())?;
    let project = doc
        .get_mut("project")
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or_else(|| "no [project] table".to_owned())?;
    let item = project
        .entry("dependencies")
        .or_insert_with(|| toml_edit::value(toml_edit::Array::new()));
    let Some(array) = item.as_array_mut() else {
        return Err("[project].dependencies is not an array".to_owned());
    };
    let requirement = format!("{package}>={version}");
    let multiline = array.iter().any(|value| {
        value
            .decor()
            .prefix()
            .and_then(|prefix| prefix.as_str())
            .is_some_and(|prefix| prefix.contains('\n'))
    });
    if multiline {
        let indent = array
            .iter()
            .last()
            .and_then(|value| value.decor().prefix())
            .and_then(|prefix| prefix.as_str())
            .unwrap_or("\n    ")
            .to_owned();
        let mut value = toml_edit::Value::from(requirement.as_str());
        value.decor_mut().set_prefix(indent);
        array.push_formatted(value);
    } else {
        array.push(requirement.as_str());
    }
    let out = doc.to_string();
    let kept = out
        .parse::<toml_edit::DocumentMut>()
        .ok()
        .and_then(|doc| {
            doc["project"]["dependencies"].as_array().map(|array| {
                array
                    .iter()
                    .any(|v| v.as_str() == Some(requirement.as_str()))
            })
        })
        .unwrap_or(false);
    if !kept {
        return Err("edit did not round-trip".to_owned());
    }
    Ok(out)
}

/// Adds `gem "package", "~> major.minor"` after the last top-level `gem` line, or at the end.
pub(super) fn insert_gem(content: &str, package: &str, version: &str) -> Result<String, String> {
    let mut parts = version.split('.');
    let requirement = match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        (Some(major), None) => major.to_owned(),
        _ => return Err("empty gem version".to_owned()),
    };
    let eol = line_ending(content);
    let mut at = content.len();
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        offset += line.len();
        if line.starts_with("gem ") {
            at = offset;
        }
    }
    let mut entry = String::new();
    if at == content.len() && !content.is_empty() && !content.ends_with('\n') {
        entry.push_str(eol);
    }
    entry.push_str(&format!("gem \"{package}\", \"~> {requirement}\"{eol}"));
    if at < content.len() && !content[..at].ends_with('\n') {
        entry.insert_str(0, eol);
    }
    Ok(splice(content, at, &entry))
}
