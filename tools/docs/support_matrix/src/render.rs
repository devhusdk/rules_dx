//! Renders the support matrix as Markdown with stable, ASCII-only output.

use std::collections::BTreeMap;

use crate::gaps::tools;
use crate::{Facts, Row, RowPlatforms, UPDATE_COMMAND};

fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("| {} |\n", header.join(" | "));
    out.push_str(&format!("|{}\n", " --- |".repeat(header.len())));
    for row in rows {
        out.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    out
}

fn rows_in<'a>(facts: &'a Facts, section: &str) -> Vec<&'a Row> {
    facts
        .rows
        .iter()
        .filter(|row| row.section == section)
        .collect()
}

fn build_table(
    facts: &Facts,
    platforms: &BTreeMap<String, RowPlatforms>,
    section: &str,
    heading: &str,
) -> String {
    let rows: Vec<Vec<String>> = rows_in(facts, section)
        .into_iter()
        .map(|row| {
            let tested = platforms.get(&row.name).cloned().unwrap_or_default();
            let declared = facts.platform_not_applicable.get(&row.name);
            // Group the unsupported platforms by reason, in platform order.
            let mut groups: Vec<(String, Vec<String>)> = Vec::new();
            for platform in &facts.platforms {
                let reason = match declared.and_then(|d| d.get(&platform.id)) {
                    Some(reason) => reason.clone(),
                    None if tested.build.contains(&platform.id) => continue,
                    None => "not tested yet".to_owned(),
                };
                let id = format!("`{}`", platform.id);
                match groups.iter_mut().find(|(r, _)| *r == reason) {
                    Some((_, ids)) => ids.push(id),
                    None => groups.push((reason, vec![id])),
                }
            }
            let cell = if groups.is_empty() {
                "none".to_owned()
            } else {
                groups
                    .iter()
                    .map(|(reason, ids)| format!("{}: {reason}", ids.join(", ")))
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            vec![row.title.clone(), cell]
        })
        .collect();
    table(&[heading, "Not supported on"], &rows)
}

fn quality_cell(facts: &Facts, row: &Row, capability: &str) -> String {
    let found = tools(facts, &row.families, capability);
    if found.is_empty() {
        if crate::gaps::not_applicable(facts, &row.families, capability) {
            return "n/a".to_owned();
        }
        return "no".to_owned();
    }
    found
        .iter()
        .map(|tool| {
            let unsupported = facts.support.iter().any(|r| {
                r.tool == *tool
                    && r.capability == capability
                    && row.families.contains(&r.family)
                    && r.platforms.is_empty()
            });
            let mut cell = format!("`{tool}`");
            if unsupported {
                cell.push_str(" unsupported");
            }
            if let Some(declared) = facts.tool_not_applicable.get(tool) {
                let notes: Vec<String> = facts
                    .platforms
                    .iter()
                    .filter_map(|p| declared.get(&p.id).map(|reason| (p, reason)))
                    .map(|(p, reason)| format!("`{}`: {reason}", p.id))
                    .collect();
                if !notes.is_empty() {
                    cell.push_str(&format!(" (not on {})", notes.join("; ")));
                }
            }
            cell
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reports whether `row` has a quality row: every language and format, and targets with a family.
pub fn shows_quality(row: &Row) -> bool {
    row.section != "targets" || !row.families.is_empty()
}

fn quality_table(facts: &Facts) -> String {
    let rows: Vec<Vec<String>> = facts
        .sections
        .iter()
        .flat_map(|section| rows_in(facts, section))
        .filter(|row| shows_quality(row))
        .map(|row| {
            vec![
                row.title.clone(),
                quality_cell(facts, row, "lint"),
                quality_cell(facts, row, "format"),
                quality_cell(facts, row, "typecheck"),
            ]
        })
        .collect();
    table(&["Name", "Lint", "Format", "Typecheck"], &rows)
}

/// Renders the whole page.
pub fn render(facts: &Facts, platforms: &BTreeMap<String, RowPlatforms>) -> String {
    let ids: Vec<String> = facts
        .platforms
        .iter()
        .map(|platform| format!("`{}`", platform.id))
        .collect();
    let mut out = String::new();
    out.push_str("# Support matrix\n\n");
    out.push_str(
        "This page lists, for each language and target, the platforms it does not support, \
         and its lint, format, and typecheck tools.\n\n",
    );
    out.push_str(
        "Everything builds and tests on every platform it supports. \
         `none` means all five platforms are supported. Missing tests on a supported platform are gaps, \
         tracked as issues and listed by `bazel run //tools/docs/support_matrix:gaps`.\n\n",
    );
    out.push_str("`n/a` means the capability does not apply to that language or format.\n\n");
    out.push_str(&format!("The platforms are {}.\n\n", ids.join(", ")));
    out.push_str(&format!(
        "Do not edit this page. Run `{UPDATE_COMMAND}` to refresh it.\n\n"
    ));
    out.push_str("## Languages\n\n");
    out.push_str(&build_table(facts, platforms, "languages", "Language"));
    out.push_str("\n## Targets\n\n");
    out.push_str(&build_table(facts, platforms, "targets", "Target"));
    out.push_str("\n## Quality\n\n");
    out.push_str(
        "Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs. \
         A tool runs on every platform its row supports unless a note says otherwise.\n\n",
    );
    out.push_str(&quality_table(facts));
    out.push_str(&not_applicable_section(facts));
    out
}

fn not_applicable_section(facts: &Facts) -> String {
    let mut out = String::new();
    for (capability, families) in &facts.not_applicable {
        if families.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str("\n### Not applicable\n\n");
        } else {
            out.push('\n');
        }
        out.push_str(&format!("Not applicable for `{capability}`:\n\n"));
        for (family, reason) in families {
            out.push_str(&format!("- `{family}`: {reason}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_facts;

    const FACTS: &str = r#"{
      "schema_version": 1,
      "platforms": [
        {"id": "linux_x86_64", "os": "linux", "cpu": "x86_64"},
        {"id": "macos_arm64", "os": "macos", "cpu": "arm64"}
      ],
      "sections": ["languages", "targets", "formats"],
      "rows": [
        {"name": "go", "title": "Go", "section": "languages", "families": ["go"],
         "ci_tests": [], "consumer_tests": [], "workflows": []},
        {"name": "yaml", "title": "YAML", "section": "formats", "families": ["yaml"],
         "ci_tests": [], "consumer_tests": [], "workflows": []}
      ],
      "quality": {"go": {"lint": ["govet"], "format": ["gofumpt"], "typecheck": []},
                  "yaml": {"lint": ["yamllint"], "format": [], "typecheck": []}},
      "support": [{"family": "yaml", "capability": "lint", "tool": "yamllint", "platforms": ["linux_x86_64"]}],
      "tool_not_applicable": {"yamllint": {"macos_arm64": "no build"}},
      "deferred_families": [],
      "not_applicable": {"typecheck": {"yaml": "data format"}},
      "not_languages": [],
      "gap_issues": {},
      "gap_enforcement": false,
      "computed_constraints": {}
    }"#;

    #[test]
    fn separates_not_applicable_capability_blocks() {
        let mut facts = parse_facts(FACTS).expect("facts");
        facts
            .not_applicable
            .entry("format".to_owned())
            .or_default()
            .insert("yaml".to_owned(), "data".to_owned());
        let out = not_applicable_section(&facts);
        assert!(
            out.contains("- `yaml`: data\n\nNot applicable for `typecheck`:"),
            "{out}"
        );
    }

    #[test]
    fn renders_stable_ascii_tables() {
        let facts = parse_facts(FACTS).expect("facts");
        let platforms = BTreeMap::from([(
            "go".to_owned(),
            RowPlatforms {
                build: vec!["linux_x86_64".to_owned()],
                consumer: Vec::new(),
            },
        )]);
        let page = render(&facts, &platforms);
        assert_eq!(page, render(&facts, &platforms));
        assert!(page.is_ascii());
        assert!(
            page.contains("| Go | `macos_arm64`: not tested yet |\n"),
            "{page}"
        );
        assert!(
            page.contains("| YAML | `yamllint` (not on `macos_arm64`: no build) | no | n/a |\n"),
            "{page}"
        );
        assert!(!page.to_lowercase().contains("experimental"));
    }
}
