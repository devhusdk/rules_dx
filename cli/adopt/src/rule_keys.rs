use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

/// Rule ID to setting, read from the rule keys of a tool's base.
pub type RuleSettings = BTreeMap<String, String>;

/// The rule settings under `keys` in a base of format `ext` (`toml`, `json` or `yaml`).
/// None when the text does not parse or the format has no extractor.
pub fn rule_settings(ext: &str, keys: &[&str], text: &str) -> Option<RuleSettings> {
    let root = match ext {
        "toml" => serde_json::to_value(text.parse::<toml::Table>().ok()?).ok()?,
        "json" => serde_json::from_str(text).ok()?,
        "yaml" => yaml_value(text),
        _ => return None,
    };
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for key in keys {
        let Some(value) = key.split('.').try_fold(&root, |node, part| node.get(part)) else {
            continue;
        };
        let label = key.rsplit('.').next().unwrap_or(key);
        collect(value, &mut Vec::new(), label, &mut found);
    }
    Some(
        found
            .into_iter()
            .map(|(rule, settings)| (rule, settings.into_iter().collect::<Vec<_>>().join(", ")))
            .collect(),
    )
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn collect(
    value: &Value,
    path: &mut Vec<String>,
    label: &str,
    found: &mut BTreeMap<String, BTreeSet<String>>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                path.push(key.clone());
                collect(child, path, label, found);
                path.pop();
            }
        }
        Value::Array(items) => {
            let setting = std::iter::once(label).chain(path.iter().map(String::as_str));
            let setting = setting.collect::<Vec<_>>().join(" ");
            for item in items {
                found
                    .entry(scalar(item))
                    .or_default()
                    .insert(setting.clone());
            }
        }
        scalar_value if !path.is_empty() => {
            found
                .entry(path.join("/"))
                .or_default()
                .insert(scalar(scalar_value));
        }
        _ => {}
    }
}

fn yaml_scalar(text: &str) -> Value {
    let text = text.trim();
    let quoted = text.len() >= 2
        && (text.starts_with('"') && text.ends_with('"')
            || text.starts_with('\'') && text.ends_with('\''));
    Value::String(if quoted { &text[1..text.len() - 1] } else { text }.to_owned())
}

/// Nested `key: value` mappings of a yaml file; lists and flow values stay plain text.
fn yaml_value(text: &str) -> Value {
    let lines: Vec<(usize, &str)> = text
        .lines()
        .filter_map(|line| {
            let body = line.split(" #").next().unwrap_or(line).trim_end();
            let trimmed = body.trim_start();
            (!trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("---"))
                .then_some((body.len() - trimmed.len(), trimmed))
        })
        .collect();
    yaml_block(&lines, &mut 0, 0)
}

fn yaml_block(lines: &[(usize, &str)], at: &mut usize, indent: usize) -> Value {
    let mut map = serde_json::Map::new();
    while let Some(&(depth, line)) = lines.get(*at) {
        if depth < indent {
            break;
        }
        *at += 1;
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let key = yaml_scalar(key);
        let key = scalar(&key);
        let child = if rest.trim().is_empty() {
            match lines.get(*at) {
                Some(&(next, _)) if next > depth => yaml_block(lines, at, next),
                _ => Value::Null,
            }
        } else {
            yaml_scalar(rest)
        };
        map.insert(key, child);
    }
    Value::Object(map)
}

fn disabled(setting: &str) -> bool {
    matches!(
        setting,
        "off" | "disable" | "disabled" | "false" | "ignore" | "0" | "\"off\""
    )
}

/// The `N rules enabled, M disabled, K changed` line and one indented line per rule,
/// or nothing when no rule differs.
pub fn rule_report(tool: &str, old: &RuleSettings, new: &RuleSettings) -> Vec<String> {
    let mut lines = Vec::new();
    let (mut enabled, mut off, mut changed) = (0, 0, 0);
    for rule in old.keys().chain(new.keys()).collect::<BTreeSet<_>>() {
        match (old.get(rule), new.get(rule)) {
            (None, Some(after)) if !disabled(after) => {
                enabled += 1;
                lines.push(format!("  + {rule} ({after})"));
            }
            (None, Some(after)) => {
                off += 1;
                lines.push(format!("  - {rule} ({after})"));
            }
            (Some(before), None) if !disabled(before) => {
                off += 1;
                lines.push(format!("  - {rule} (removed)"));
            }
            (Some(before), Some(after)) if before != after => {
                if disabled(before) && !disabled(after) {
                    enabled += 1;
                    lines.push(format!("  + {rule} ({after})"));
                } else if disabled(after) && !disabled(before) {
                    off += 1;
                    lines.push(format!("  - {rule} ({after})"));
                } else {
                    changed += 1;
                    lines.push(format!("  ~ {rule}: {before} -> {after}"));
                }
            }
            _ => {}
        }
    }
    if lines.is_empty() {
        return lines;
    }
    lines.insert(
        0,
        format!("{tool}: {enabled} rules enabled, {off} disabled, {changed} changed"),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(entries: &[(&str, &str)]) -> RuleSettings {
        entries
            .iter()
            .map(|(rule, setting)| ((*rule).to_owned(), (*setting).to_owned()))
            .collect()
    }

    #[test]
    fn toml_reads_lists_and_per_file_tables() {
        let text = "[lint]\nselect = [\"E\", \"F401\"]\nignore = [\"E501\"]\n\n[lint.per-file-ignores]\n\"t.py\" = [\"S101\"]\n";
        let keys = ["lint.select", "lint.ignore", "lint.per-file-ignores"];
        assert_eq!(
            rule_settings("toml", &keys, text),
            Some(settings(&[
                ("E", "select"),
                ("F401", "select"),
                ("E501", "ignore"),
                ("S101", "per-file-ignores t.py"),
            ]))
        );
        assert_eq!(rule_settings("toml", &keys, "= bad"), None);
    }

    #[test]
    fn json_reads_nested_rule_groups() {
        let text = r#"{"linter": {"rules": {"preset": "none", "a11y": {"useAltText": "error", "noAutofocus": "off"}}}}"#;
        assert_eq!(
            rule_settings("json", &["linter.rules"], text),
            Some(settings(&[
                ("preset", "none"),
                ("a11y/useAltText", "error"),
                ("a11y/noAutofocus", "off"),
            ]))
        );
    }

    #[test]
    fn yaml_reads_rule_mappings() {
        let text = "# c\nrules:\n  anchors: enable\n  truthy:\n    check-keys: false\n  line-length: 'disable'\nother: x\n";
        assert_eq!(
            rule_settings("yaml", &["rules"], text),
            Some(settings(&[
                ("anchors", "enable"),
                ("truthy/check-keys", "false"),
                ("line-length", "disable"),
            ]))
        );
        assert_eq!(rule_settings("mjs", &["rules"], text), None);
    }

    #[test]
    fn report_counts_enabled_disabled_and_changed() {
        let old = settings(&[("a", "error"), ("b", "error"), ("c", "warn")]);
        let new = settings(&[("a", "off"), ("c", "error"), ("d", "error")]);
        assert_eq!(
            rule_report("biome", &old, &new),
            [
                "biome: 1 rules enabled, 2 disabled, 1 changed",
                "  - a (off)",
                "  - b (removed)",
                "  ~ c: warn -> error",
                "  + d (error)",
            ]
        );
        assert!(rule_report("biome", &old, &old).is_empty());
    }
}
