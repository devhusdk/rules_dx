//! Severity by rule category, mapped once per tool in `core/quality/tools.bzl`.

use crate::ToolSeverity;
pub use crate::severity_gen::TOOL_SEVERITIES;

pub struct ToolSeverityMap {
    pub default: Option<ToolSeverity>,
    /// The lowest severity a rule with no category and no default may report.
    pub floor: Option<ToolSeverity>,
    pub categories: &'static [(&'static str, ToolSeverity)],
}

fn rank(level: ToolSeverity) -> u8 {
    match level {
        ToolSeverity::Info => 0,
        ToolSeverity::Warning => 1,
        ToolSeverity::Error => 2,
    }
}

/// Returns the mapped severity for a rule; with no matching entry, `parsed` raised to the tool's floor.
#[must_use]
pub fn resolve(
    table: &[(&str, ToolSeverityMap)],
    tool_id: &str,
    rule_id: &str,
    parsed: ToolSeverity,
) -> ToolSeverity {
    let Some((_, map)) = table.iter().find(|(id, _)| *id == tool_id) else {
        return parsed;
    };
    map.categories
        .iter()
        .filter(|(prefix, _)| rule_id.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, level)| *level)
        .or(map.default)
        .unwrap_or_else(|| match map.floor {
            Some(floor) if rank(floor) > rank(parsed) => floor,
            Some(_) | None => parsed,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUFF: ToolSeverityMap = ToolSeverityMap {
        default: None,
        floor: None,
        categories: &[
            ("E", ToolSeverity::Error),
            ("E5", ToolSeverity::Warning),
            ("D", ToolSeverity::Info),
        ],
    };

    #[test]
    fn unmapped_tool_keeps_the_parser_severity() {
        assert_eq!(
            resolve(&[], "ruff", "E501", ToolSeverity::Info),
            ToolSeverity::Info
        );
        assert_eq!(
            resolve(TOOL_SEVERITIES, "no-such-tool", "X1", ToolSeverity::Info),
            ToolSeverity::Info
        );
    }

    #[test]
    fn example_mapping_sets_error() {
        let table = [("ruff", RUFF)];
        assert_eq!(
            resolve(&table, "ruff", "E101", ToolSeverity::Warning),
            ToolSeverity::Error
        );
    }

    #[test]
    fn longest_prefix_wins() {
        let table = [("ruff", RUFF)];
        assert_eq!(
            resolve(&table, "ruff", "E501", ToolSeverity::Error),
            ToolSeverity::Warning
        );
    }

    #[test]
    fn no_match_keeps_parser_severity_and_other_tools_are_untouched() {
        let table = [("ruff", RUFF)];
        assert_eq!(
            resolve(&table, "ruff", "F401", ToolSeverity::Error),
            ToolSeverity::Error
        );
        assert_eq!(
            resolve(&table, "flake8", "E101", ToolSeverity::Warning),
            ToolSeverity::Warning
        );
    }

    #[test]
    fn default_applies_when_no_prefix_matches() {
        let table = [(
            "t",
            ToolSeverityMap {
                default: Some(ToolSeverity::Info),
                floor: None,
                categories: &[("a", ToolSeverity::Error)],
            },
        )];
        assert_eq!(
            resolve(&table, "t", "b1", ToolSeverity::Error),
            ToolSeverity::Info
        );
        assert_eq!(
            resolve(&table, "t", "a1", ToolSeverity::Info),
            ToolSeverity::Error
        );
    }

    const FLOORED: ToolSeverityMap = ToolSeverityMap {
        default: None,
        floor: Some(ToolSeverity::Warning),
        categories: &[("hint", ToolSeverity::Info)],
    };

    #[test]
    fn floor_raises_info_but_keeps_higher_levels() {
        let table = [("t", FLOORED)];
        assert_eq!(
            resolve(&table, "t", "r1", ToolSeverity::Info),
            ToolSeverity::Warning
        );
        assert_eq!(
            resolve(&table, "t", "r1", ToolSeverity::Error),
            ToolSeverity::Error
        );
    }

    #[test]
    fn mapped_category_wins_over_the_floor() {
        let table = [("t", FLOORED)];
        assert_eq!(
            resolve(&table, "t", "hint-x", ToolSeverity::Warning),
            ToolSeverity::Info
        );
    }

    /// Parsers that may report `Info` as a hint by design, and why.
    const HINT_PARSERS: &[(&str, &str)] = &[
        ("hlint", "hlint Suggestion findings are optional rewrites"),
        ("vale", "vale suggestion-level rules are style hints"),
    ];

    /// Shared parsers whose callers pass the tool id, and why their `Info` is fine.
    const SHARED_PARSERS: &[(&str, &str)] = &[
        (
            "mod.rs",
            "the rubocop-style parser maps per tool through TOOL_SEVERITIES",
        ),
        ("jvm_diagnostics.rs", "compiler notes are not rule findings"),
        (
            "sarif.rs",
            "SARIF note levels map per tool through TOOL_SEVERITIES",
        ),
    ];

    fn parser_sources() -> Vec<(String, String)> {
        let anchor = dx_testing::env_file("DX_PARSER_MOD");
        let dir = anchor.parent().expect("parsers dir");
        let mut sources: Vec<(String, String)> = std::fs::read_dir(dir)
            .expect("read parsers dir")
            .map(|entry| entry.expect("parser entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .map(|path| {
                let name = path
                    .file_name()
                    .expect("file name")
                    .to_string_lossy()
                    .into_owned();
                let text = std::fs::read_to_string(&path).expect("read parser");
                let shipped = text
                    .split("#[cfg(test)]")
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                (name, shipped)
            })
            .collect();
        sources.sort();
        assert!(sources.len() > 100, "found only {} parsers", sources.len());
        sources
    }

    fn tool_const(text: &str) -> Option<&str> {
        let rest = text.split("const TOOL: &str = \"").nth(1)?;
        rest.split('"').next()
    }

    #[test]
    fn every_parser_that_reports_info_has_a_severity_map() {
        let mut leaks = Vec::new();
        for (name, text) in parser_sources() {
            if !text.contains("ToolSeverity::Info") {
                continue;
            }
            let covered = match tool_const(&text) {
                Some(tool) => {
                    TOOL_SEVERITIES.iter().any(|(id, _)| *id == tool)
                        || HINT_PARSERS.iter().any(|(id, _)| *id == tool)
                }
                None => SHARED_PARSERS.iter().any(|(file, _)| *file == name),
            };
            if !covered {
                leaks.push(name);
            }
        }
        assert!(
            leaks.is_empty(),
            "parsers report Info with no severity map; add severity(floor = \"warning\") to the tool: {leaks:?}"
        );
    }

    #[test]
    fn no_parser_declares_a_level_rule_list() {
        let lists: Vec<String> = parser_sources()
            .into_iter()
            .filter(|(_, text)| {
                [
                    "INFO_RULES",
                    "WARNING_RULES",
                    "ERROR_RULES",
                    "NOTE_RULES",
                    "HINT_RULES",
                ]
                .iter()
                .any(|name| text.contains(&format!("const {name}")))
            })
            .map(|(name, _)| name)
            .collect();
        assert!(
            lists.is_empty(),
            "set rule levels with severity() categories in the tool's .bzl, not in the parser: {lists:?}"
        );
    }
}
