//! Severity by rule category, mapped once per tool in `quality/tools.bzl`.

use crate::ToolSeverity;
pub use crate::severity_gen::TOOL_SEVERITIES;

pub struct ToolSeverityMap {
    pub default: Option<ToolSeverity>,
    pub categories: &'static [(&'static str, ToolSeverity)],
}

/// Returns the mapped severity for a rule, or `parsed` when the tool has no matching entry.
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
        .unwrap_or(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUFF: ToolSeverityMap = ToolSeverityMap {
        default: None,
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
}
