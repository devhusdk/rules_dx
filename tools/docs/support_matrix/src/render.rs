//! Renders the support matrix as Markdown with stable, ASCII-only output.

use std::collections::BTreeMap;

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

fn unsupported_cell(
    facts: &Facts,
    platforms: &BTreeMap<String, RowPlatforms>,
    row: &Row,
) -> String {
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
    if groups.is_empty() {
        "none".to_owned()
    } else {
        groups
            .iter()
            .map(|(reason, ids)| format!("{}: {reason}", ids.join(", ")))
            .collect::<Vec<_>>()
            .join("; ")
    }
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
            let cell = unsupported_cell(facts, platforms, row);
            vec![row.title.clone(), cell]
        })
        .collect();
    table(&[heading, "Not supported on"], &rows)
}

fn compiler_table(facts: &Facts, platforms: &BTreeMap<String, RowPlatforms>) -> String {
    let rows: Vec<Vec<String>> = rows_in(facts, "compilers")
        .into_iter()
        .map(|row| {
            let field = |key: &str| match row.identity.get(key).map(String::as_str) {
                Some("") | None => "no".to_owned(),
                Some(value) => value.to_owned(),
            };
            vec![
                row.title.clone(),
                field("driver"),
                field("coverage"),
                unsupported_cell(facts, platforms, row),
            ]
        })
        .collect();
    let mut out = table(
        &["Compiler", "Driver", "Coverage", "Not supported on"],
        &rows,
    );
    for (compiler, reason) in &facts.pending_compilers {
        out.push_str(&format!("\nPending `{compiler}`: {reason}\n"));
    }
    out
}

/// Returns the quality tools of `capability` across `families`, sorted and unique.
fn tools(facts: &Facts, families: &[String], capability: &str) -> Vec<String> {
    let mut out: Vec<String> = families
        .iter()
        .filter_map(|family| facts.quality.get(family))
        .filter_map(|caps| caps.get(capability))
        .flatten()
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Reports whether `capability` never applies to a row: it has no tools and
/// every one of its families is declared not applicable.
fn not_applicable(facts: &Facts, families: &[String], capability: &str) -> bool {
    let Some(declared) = facts.not_applicable.get(capability) else {
        return false;
    };
    !families.is_empty()
        && tools(facts, families, capability).is_empty()
        && families.iter().all(|family| declared.contains_key(family))
}

fn quality_cell(facts: &Facts, row: &Row, capability: &str) -> String {
    let found = tools(facts, &row.families, capability);
    if found.is_empty() {
        if not_applicable(facts, &row.families, capability) {
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
            let declared = row.families.iter().find_map(|family| {
                facts
                    .tool_not_applicable
                    .get(family)
                    .and_then(|by_capability| by_capability.get(capability))
                    .and_then(|by_tool| by_tool.get(tool))
            });
            if let Some(declared) = declared {
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

/// Reports whether `row` has a quality row: every language and format, and targets with a family; never compilers.
pub fn shows_quality(row: &Row) -> bool {
    match row.section.as_str() {
        "targets" => !row.families.is_empty(),
        "compilers" => false,
        _ => true,
    }
}

fn doc_cell(row: &Row) -> String {
    match row.doc.as_deref() {
        None => "n/a".to_owned(),
        Some("") => "no".to_owned(),
        Some(tool) => format!("`{tool}`"),
    }
}

/// Names the collector that writes the row's LCOV with its note, or `no` with the reason it has none.
fn coverage_cell(row: &Row) -> String {
    match &row.coverage {
        None => "n/a".to_owned(),
        Some(coverage) if coverage.collectors.is_empty() => format!("no: {}", coverage.reason),
        Some(coverage) => {
            let collectors = coverage
                .collectors
                .iter()
                .map(|collector| format!("`{collector}`"))
                .collect::<Vec<_>>()
                .join(", ");
            if coverage.reason.is_empty() {
                collectors
            } else {
                format!("{collectors} ({})", coverage.reason)
            }
        }
    }
}

/// Names the unused-code tools of the row, or `no` when it has none yet.
fn dead_code_cell(row: &Row) -> String {
    match &row.dead_code {
        None => "n/a".to_owned(),
        Some(tools) if tools.is_empty() => "no".to_owned(),
        Some(tools) => tools
            .iter()
            .map(|tool| {
                if tool == "none" {
                    tool.clone()
                } else {
                    format!("`{tool}`")
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
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
                doc_cell(row),
                coverage_cell(row),
                dead_code_cell(row),
            ]
        })
        .collect();
    table(
        &[
            "Name",
            "Lint",
            "Format",
            "Typecheck",
            "API docs",
            "Coverage",
            "Dead code",
        ],
        &rows,
    )
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
        "This page lists, for each language, target, and compiler, the platforms it does not support, \
         and its lint, format, typecheck, and API doc tools.\n\n",
    );
    out.push_str(
        "Everything builds and tests on every platform it supports. \
         `none` means all five platforms are supported.\n\n",
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
    out.push_str("\n## Compilers\n\n");
    out.push_str("Select one with `--@rules_dx//cc/toolchains:compiler=<value>`.\n\n");
    out.push_str(&compiler_table(facts, platforms));
    out.push_str("\n## Quality\n\n");
    out.push_str(
        "Each cell names the tools `dx lint`, `dx format`, or `dx typecheck` runs, and the tool that builds its API docs. \
         A tool runs on every platform its row supports unless a note says otherwise. \
         Coverage names the collector that writes the LCOV for the row's tests, or `no` with the reason it has none. \
         Dead code names the tool and rule that report unused code, `no` when none runs yet, or `none` when the language has no such tool.\n\n",
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
      "platforms": [
        {"id": "linux_x86_64", "os": "linux", "cpu": "x86_64"},
        {"id": "macos_arm64", "os": "macos", "cpu": "arm64"}
      ],
      "sections": ["languages", "targets", "compilers", "formats"],
      "rows": [
        {"name": "go", "title": "Go", "section": "languages", "families": ["go"],
         "coverage": {"collectors": ["go cover"], "reason": ""},
         "dead_code": [],
         "ci_tests": [], "consumer_tests": [], "workflows": []},
        {"name": "cc_llvm", "title": "LLVM", "section": "compilers", "families": [],
         "identity": {"driver": "gnu", "coverage": ""},
         "ci_tests": [], "consumer_tests": [], "workflows": []},
        {"name": "yaml", "title": "YAML", "section": "formats", "families": ["yaml"],
         "coverage": {"collectors": [], "reason": "no collector"},
         "ci_tests": [], "consumer_tests": [], "workflows": []}
      ],
      "quality": {"go": {"lint": ["govet"], "format": ["gofumpt"], "typecheck": []},
                  "yaml": {"lint": ["yamllint"], "format": [], "typecheck": []}},
      "support": [{"family": "yaml", "capability": "lint", "tool": "yamllint", "platforms": ["linux_x86_64"]}],
      "tool_not_applicable": {"yaml": {"lint": {"yamllint": {"macos_arm64": "no build"}}}},
      "deferred_families": [],
      "not_applicable": {"typecheck": {"yaml": "data format"}},
      "not_languages": [],
      "pending_compilers": {"msvc": "waiting on #5954"},
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
            page.contains("| Go | `govet` | `gofumpt` | no | n/a | `go cover` | no |\n"),
            "{page}"
        );
        assert!(
            page.contains(
                "| YAML | `yamllint` (not on `macos_arm64`: no build) | no | n/a | n/a | no: no collector | n/a |\n"
            ),
            "{page}"
        );
        assert!(!page.to_lowercase().contains("experimental"));
    }

    #[test]
    fn appends_the_coverage_note_to_the_collector() {
        let mut facts = parse_facts(FACTS).expect("facts");
        facts.rows[0].coverage.as_mut().expect("coverage").reason = "runs on Linux only".to_owned();
        assert_eq!(
            coverage_cell(&facts.rows[0]),
            "`go cover` (runs on Linux only)"
        );
    }

    #[test]
    fn lists_compilers_outside_quality() {
        let facts = parse_facts(FACTS).expect("facts");
        let page = render(&facts, &BTreeMap::new());
        let (head, quality) = page.split_once("## Quality").expect("quality");
        assert!(head.contains("## Compilers\n"), "{page}");
        assert!(
            head.contains("| Compiler | Driver | Coverage | Not supported on |"),
            "{page}"
        );
        assert!(head.contains("| LLVM | gnu | no |"), "{page}");
        assert!(head.contains("Pending `msvc`: waiting on #5954"), "{page}");
        assert!(!quality.contains("LLVM"), "{page}");
    }
}
