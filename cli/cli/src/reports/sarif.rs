use std::collections::{BTreeMap, BTreeSet};

use super::ReportError;
use dx_output::{compare_diagnostics, DiagnosticEvent, Severity};
use line_index::{LineIndex, TextSize, WideEncoding};
use serde_sarif::sarif::{
    ArtifactLocation, Invocation, Location, Message, PhysicalLocation, Region, ReportingDescriptor,
    Result as SarifResult, ResultLevel, Run, Sarif, Tool, ToolComponent,
};

pub fn byte_to_line(path: &str, text: &str, offset: u64) -> Result<(u64, u64), ReportError> {
    let bad = || ReportError::BadOffset {
        path: path.to_owned(),
        offset,
    };
    if offset > text.len() as u64 {
        return Err(bad());
    }
    let offset_usize = offset as usize;
    if !text.is_char_boundary(offset_usize) {
        return Err(bad());
    }
    if text.len() >= u32::MAX as usize {
        let prefix = &text[..offset_usize];
        let line = prefix.as_bytes().iter().filter(|&&b| b == b'\n').count() as u64 + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() as u64 + 1;
        return Ok((line, column));
    }
    let index = LineIndex::new(text);
    let size = TextSize::from(offset as u32);
    let line_col = index.try_line_col(size).ok_or_else(bad)?;
    let wide = index
        .to_wide(WideEncoding::Utf32, line_col)
        .ok_or_else(bad)?;
    Ok((wide.line as u64 + 1, wide.col as u64 + 1))
}

fn check_shape(finding: &DiagnosticEvent) -> Result<(), ReportError> {
    if finding.tool.is_empty() {
        return Err(ReportError::InvalidFinding {
            detail: "empty tool",
        });
    }
    if finding.message.is_empty() {
        return Err(ReportError::InvalidFinding {
            detail: "empty message",
        });
    }
    if finding.path.is_none() && finding.range.is_some() {
        return Err(ReportError::RangeWithoutPath);
    }
    if let Some((start, end)) = finding.range {
        if start > end {
            return Err(ReportError::InvertedRange);
        }
    }
    Ok(())
}

fn sarif_level(severity: Severity) -> ResultLevel {
    match severity {
        Severity::Error => ResultLevel::Error,
        Severity::Warning => ResultLevel::Warning,
        Severity::Info => ResultLevel::Note,
    }
}

fn location(
    finding: &DiagnosticEvent,
    snapshots: &BTreeMap<String, String>,
) -> Result<Option<Location>, ReportError> {
    let Some(path) = &finding.path else {
        if finding.range.is_some() {
            return Err(ReportError::RangeWithoutPath);
        }
        return Ok(None);
    };
    let artifact = ArtifactLocation::builder().uri(path.clone()).build();
    if let Some((start, end)) = finding.range {
        if start > end {
            return Err(ReportError::InvertedRange);
        }
        let text = snapshots
            .get(path)
            .ok_or_else(|| ReportError::MissingSnapshot { path: path.clone() })?;
        let (start_line, start_column) = byte_to_line(path, text, start)?;
        let (end_line, end_column) = byte_to_line(path, text, end)?;
        let region = Region::builder()
            .start_line(start_line as i64)
            .start_column(start_column as i64)
            .end_line(end_line as i64)
            .end_column(end_column as i64)
            .build();
        let physical = PhysicalLocation::builder()
            .artifact_location(artifact)
            .region(region)
            .build();
        Ok(Some(
            Location::builder().physical_location(physical).build(),
        ))
    } else {
        let physical = PhysicalLocation::builder()
            .artifact_location(artifact)
            .build();
        Ok(Some(
            Location::builder().physical_location(physical).build(),
        ))
    }
}

fn result(
    finding: &DiagnosticEvent,
    snapshots: &BTreeMap<String, String>,
) -> Result<SarifResult, ReportError> {
    if finding.tool.is_empty() {
        return Err(ReportError::InvalidFinding {
            detail: "empty tool",
        });
    }
    if finding.message.is_empty() {
        return Err(ReportError::InvalidFinding {
            detail: "empty message",
        });
    }
    let message = Message::builder().text(finding.message.clone()).build();
    let level = sarif_level(finding.severity);
    let locations = location(finding, snapshots)?.map(|single| vec![single]);
    match (&finding.rule, locations) {
        (Some(rule), Some(locations)) => Ok(SarifResult::builder()
            .message(message)
            .level(level)
            .rule_id(rule.clone())
            .locations(locations)
            .build()),
        (Some(rule), None) => Ok(SarifResult::builder()
            .message(message)
            .level(level)
            .rule_id(rule.clone())
            .build()),
        (None, Some(locations)) => Ok(SarifResult::builder()
            .message(message)
            .level(level)
            .locations(locations)
            .build()),
        (None, None) => Ok(SarifResult::builder().message(message).level(level).build()),
    }
}

pub fn render_sarif(
    tools: &[String],
    findings: &[DiagnosticEvent],
    snapshots: &BTreeMap<String, String>,
    complete: bool,
) -> Result<String, ReportError> {
    render_sarif_with_baseline(tools, findings, &[], snapshots, complete)
}

pub fn render_sarif_with_baseline(
    tools: &[String],
    findings: &[DiagnosticEvent],
    suppressed: &[bool],
    snapshots: &BTreeMap<String, String>,
    complete: bool,
) -> Result<String, ReportError> {
    let ordered: BTreeSet<&str> = tools.iter().map(String::as_str).collect();
    let flags: Vec<bool> = if suppressed.len() == findings.len() {
        suppressed.to_vec()
    } else {
        vec![false; findings.len()]
    };
    let mut paired: Vec<(&DiagnosticEvent, bool)> = findings.iter().zip(flags).collect();
    paired.sort_by(|a, b| compare_diagnostics(a.0, b.0).then_with(|| a.1.cmp(&b.1)));
    let mut by_tool: BTreeMap<&str, Vec<(&DiagnosticEvent, bool)>> = BTreeMap::new();
    for pair in &paired {
        check_shape(pair.0)?;
        if !ordered.contains(pair.0.tool.as_str()) {
            return Err(ReportError::UnknownTool {
                tool: pair.0.tool.clone(),
            });
        }
        by_tool
            .entry(pair.0.tool.as_str())
            .or_default()
            .push(*pair);
    }
    let mut runs = Vec::with_capacity(ordered.len());
    let mut suppressed_positions: Vec<Vec<usize>> = Vec::with_capacity(ordered.len());
    for tool in ordered {
        let tool_findings = by_tool.remove(tool).unwrap_or_default();
        let rules: BTreeSet<&str> = tool_findings
            .iter()
            .filter_map(|(finding, _)| finding.rule.as_deref())
            .collect();
        let mut results = Vec::with_capacity(tool_findings.len());
        let mut positions = Vec::new();
        for (finding, flag) in tool_findings {
            results.push(result(finding, snapshots)?);
            if flag {
                positions.push(results.len() - 1);
            }
        }
        let descriptors = rules
            .into_iter()
            .map(|rule| ReportingDescriptor::builder().id(rule.to_owned()).build())
            .collect::<Vec<_>>();
        let driver = ToolComponent::builder()
            .name(tool.to_owned())
            .rules(descriptors)
            .build();
        let tool_value = Tool::builder().driver(driver).build();
        if complete {
            runs.push(Run::builder().tool(tool_value).results(results).build());
        } else {
            let invocation = Invocation::builder().execution_successful(false).build();
            runs.push(
                Run::builder()
                    .tool(tool_value)
                    .results(results)
                    .invocations(vec![invocation])
                    .build(),
            );
        }
        suppressed_positions.push(positions);
    }
    let document = Sarif::builder()
        .schema("https://json.schemastore.org/sarif-2.1.0.json".to_owned())
        .runs(runs)
        .version(serde_json::Value::String("2.1.0".to_owned()))
        .build();
    if suppressed_positions.iter().all(Vec::is_empty) {
        return Ok(dx_fingerprint::to_json(&document)?);
    }
    let mut value = serde_json::to_value(&document)
        .map_err(|error| ReportError::SarifRender {
            detail: error.to_string(),
        })?;
    for (run_index, positions) in suppressed_positions.iter().enumerate() {
        for result_index in positions {
            let slot = value
                .get_mut("runs")
                .and_then(|runs| runs.as_array_mut())
                .and_then(|runs| runs.get_mut(run_index))
                .and_then(|run| run.get_mut("results"))
                .and_then(|results| results.as_array_mut())
                .and_then(|results| results.get_mut(*result_index))
                .ok_or_else(|| ReportError::SarifRender {
                    detail: format!(
                        "suppressed result {result_index} is missing from run {run_index}"
                    ),
                })?;
            slot["suppressions"] = serde_json::json!([{
                "kind": "external",
                "justification": "accepted quality baseline",
            }]);
        }
    }
    Ok(dx_fingerprint::to_json(&value)?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use dx_output::Snapshot;
    use serde_json::{json, Value};

    fn finding(tool: &str, severity: Severity, path: Option<&str>) -> DiagnosticEvent {
        DiagnosticEvent {
            severity,
            tool: tool.to_owned(),
            message: format!("{tool} finding"),
            rule: Some(format!("{tool}/rule")),
            path: path.map(str::to_owned),
            range: None,
            snapshot: Snapshot::Terminal,
            fixable: false,
            resolution: None,
        }
    }

    fn ranged(path: &str, start: u64, end: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            severity: Severity::Warning,
            tool: "lint-tool".to_owned(),
            message: "ranged finding".to_owned(),
            rule: Some("lint-tool/rule".to_owned()),
            path: Some(path.to_owned()),
            range: Some((start, end)),
            snapshot: Snapshot::Terminal,
            fixable: false,
            resolution: None,
        }
    }

    #[test]
    fn byte_to_line_counts_lines_and_characters() {
        let text = "ab\nc\u{e9}d\nef";
        assert_eq!(byte_to_line("f", text, 0), Ok((1, 1)));
        assert_eq!(byte_to_line("f", text, 2), Ok((1, 3)));
        assert_eq!(byte_to_line("f", text, 3), Ok((2, 1)));
        assert_eq!(byte_to_line("f", text, 4), Ok((2, 2)));
        assert_eq!(byte_to_line("f", text, 7), Ok((2, 4)));
        assert_eq!(byte_to_line("f", text, 8), Ok((3, 1)));
        assert_eq!(byte_to_line("f", text, text.len() as u64), Ok((3, 3)));
    }

    #[test]
    fn byte_to_line_covers_empty_trailing_crlf_and_astral() {
        assert_eq!(byte_to_line("f", "", 0), Ok((1, 1)));
        let trailing = "ab\n";
        assert_eq!(byte_to_line("f", trailing, 0), Ok((1, 1)));
        assert_eq!(byte_to_line("f", trailing, 2), Ok((1, 3)));
        assert_eq!(byte_to_line("f", trailing, 3), Ok((2, 1)));
        let crlf = "a\r\nb";
        assert_eq!(byte_to_line("f", crlf, 0), Ok((1, 1)));
        assert_eq!(byte_to_line("f", crlf, 1), Ok((1, 2)));
        assert_eq!(byte_to_line("f", crlf, 2), Ok((1, 3)));
        assert_eq!(byte_to_line("f", crlf, 3), Ok((2, 1)));
        assert_eq!(byte_to_line("f", crlf, 4), Ok((2, 2)));
        let astral = "a\u{1f496}b";
        assert_eq!(byte_to_line("f", astral, 0), Ok((1, 1)));
        assert_eq!(byte_to_line("f", astral, 1), Ok((1, 2)));
        assert_eq!(byte_to_line("f", astral, 5), Ok((1, 3)));
        assert_eq!(byte_to_line("f", astral, 6), Ok((1, 4)));
    }

    #[test]
    fn byte_to_line_rejects_bad_offsets() {
        let text = "\u{e9}x";
        assert_eq!(
            byte_to_line("f", text, 1),
            Err(ReportError::BadOffset {
                path: "f".to_owned(),
                offset: 1,
            })
        );
        assert_eq!(
            byte_to_line("f", text, 99),
            Err(ReportError::BadOffset {
                path: "f".to_owned(),
                offset: 99,
            })
        );
        let astral = "a\u{1f496}b";
        for offset in [2, 3, 4] {
            assert_eq!(
                byte_to_line("f", astral, offset),
                Err(ReportError::BadOffset {
                    path: "f".to_owned(),
                    offset,
                })
            );
        }
    }

    #[test]
    fn sarif_projects_runs_rules_results_and_regions() {
        let snapshots = BTreeMap::from([("src/a.py".to_owned(), "x = 1\nxx = 2\n".to_owned())]);
        let tools = ["lint-tool".to_owned(), "other-tool".to_owned()];
        let findings = vec![
            finding("other-tool", Severity::Error, None),
            ranged("src/a.py", 0, 5),
            DiagnosticEvent {
                severity: Severity::Info,
                tool: "lint-tool".to_owned(),
                message: "note without rule".to_owned(),
                rule: None,
                path: Some("src/a.py".to_owned()),
                range: None,
                snapshot: Snapshot::Terminal,
                fixable: false,
                resolution: None,
            },
        ];
        let document: Value = serde_json::from_str(
            &render_sarif(&tools, &findings, &snapshots, true).expect("render"),
        )
        .expect("valid JSON");
        assert_eq!(document["version"], json!("2.1.0"));
        let runs = document["runs"].as_array().expect("runs");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0]["tool"]["driver"]["name"], json!("lint-tool"));
        assert_eq!(runs[1]["tool"]["driver"]["name"], json!("other-tool"));
        let lint_results = runs[0]["results"].as_array().expect("results");
        assert_eq!(lint_results.len(), 2);
        assert_eq!(lint_results[0]["level"], json!("warning"));
        assert_eq!(
            lint_results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            json!("src/a.py")
        );
        assert_eq!(
            lint_results[0]["locations"][0]["physicalLocation"]["region"],
            json!({"startLine": 1, "startColumn": 1, "endLine": 1, "endColumn": 6})
        );
        assert_eq!(lint_results[1]["level"], json!("note"));
        assert!(lint_results[1].get("ruleId").is_none());
        assert_eq!(
            lint_results[1]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            json!("src/a.py")
        );
        assert!(lint_results[1]["locations"][0]["physicalLocation"]
            .get("region")
            .is_none());
        let rules = runs[0]["tool"]["driver"]["rules"]
            .as_array()
            .expect("rules");
        assert_eq!(rules, &vec![json!({"id": "lint-tool/rule"})]);
        assert_eq!(runs[1]["results"][0]["level"], json!("error"));
        assert!(runs[1]["results"][0].get("locations").is_none());
        assert!(document.get("invocations").is_none());
        assert!(runs[0].get("invocations").is_none());
    }

    #[test]
    fn sarif_keeps_empty_runs_and_marks_partial_invocations() {
        let document: Value = serde_json::from_str(
            &render_sarif(&["lint-tool".to_owned()], &[], &BTreeMap::new(), false).expect("render"),
        )
        .expect("valid JSON");
        let runs = document["runs"].as_array().expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["results"], json!([]));
        assert_eq!(
            runs[0]["invocations"],
            json!([{"executionSuccessful": false}])
        );
        assert_eq!(runs[0]["tool"]["driver"]["rules"], json!([]));
    }

    #[test]
    fn sarif_is_deterministic_over_finding_order() {
        let snapshots = BTreeMap::new();
        let tools = ["b-tool".to_owned(), "a-tool".to_owned()];
        let first = vec![
            finding("b-tool", Severity::Error, None),
            finding("a-tool", Severity::Info, None),
        ];
        let mut second = first.clone();
        second.reverse();
        assert_eq!(
            render_sarif(&tools, &first, &snapshots, true),
            render_sarif(&tools, &second, &snapshots, true)
        );
    }

    #[test]
    fn sarif_rejects_inconsistent_inputs() {
        let snapshots = BTreeMap::new();
        let tools = ["lint-tool".to_owned()];
        assert_eq!(
            render_sarif(
                &tools,
                &[finding("other-tool", Severity::Error, None)],
                &snapshots,
                true
            ),
            Err(ReportError::UnknownTool {
                tool: "other-tool".to_owned(),
            })
        );
        assert_eq!(
            render_sarif(&tools, &[ranged("src/a.py", 0, 1)], &snapshots, true),
            Err(ReportError::MissingSnapshot {
                path: "src/a.py".to_owned(),
            })
        );
        assert_eq!(
            render_sarif(
                &tools,
                &[DiagnosticEvent {
                    range: Some((1, 0)),
                    ..ranged("src/a.py", 1, 0)
                }],
                &BTreeMap::from([("src/a.py".to_owned(), "ab".to_owned())]),
                true,
            ),
            Err(ReportError::InvertedRange)
        );
        assert_eq!(
            render_sarif(
                &tools,
                &[DiagnosticEvent {
                    range: Some((0, 1)),
                    path: None,
                    ..finding("lint-tool", Severity::Error, None)
                }],
                &snapshots,
                true,
            ),
            Err(ReportError::RangeWithoutPath)
        );
        assert_eq!(
            render_sarif(
                &tools,
                &[DiagnosticEvent {
                    tool: String::new(),
                    ..finding("lint-tool", Severity::Error, None)
                }],
                &snapshots,
                true,
            ),
            Err(ReportError::InvalidFinding {
                detail: "empty tool"
            })
        );
        assert_eq!(
            render_sarif(
                &tools,
                &[DiagnosticEvent {
                    message: String::new(),
                    ..finding("lint-tool", Severity::Error, None)
                }],
                &snapshots,
                true,
            ),
            Err(ReportError::InvalidFinding {
                detail: "empty message"
            })
        );
    }

    #[test]
    fn shape_helpers_reject_locally() {
        let snapshots = BTreeMap::new();
        assert_eq!(
            location(
                &DiagnosticEvent {
                    range: Some((0, 1)),
                    path: None,
                    ..finding("lint-tool", Severity::Error, None)
                },
                &snapshots,
            ),
            Err(ReportError::RangeWithoutPath)
        );
        assert_eq!(
            location(&finding("lint-tool", Severity::Error, None), &snapshots),
            Ok(None)
        );
        assert_eq!(
            location(
                &DiagnosticEvent {
                    range: Some((1, 0)),
                    ..ranged("src/a.py", 1, 0)
                },
                &snapshots,
            ),
            Err(ReportError::InvertedRange)
        );
        assert_eq!(
            result(
                &DiagnosticEvent {
                    tool: String::new(),
                    ..finding("lint-tool", Severity::Error, None)
                },
                &snapshots,
            ),
            Err(ReportError::InvalidFinding {
                detail: "empty tool"
            })
        );
        assert_eq!(
            result(
                &DiagnosticEvent {
                    message: String::new(),
                    ..finding("lint-tool", Severity::Error, None)
                },
                &snapshots,
            ),
            Err(ReportError::InvalidFinding {
                detail: "empty message"
            })
        );
    }

    #[test]
    fn sarif_output_parses_as_typed_sarif() {
        let snapshots = BTreeMap::from([("src/a.py".to_owned(), "x = 1\n".to_owned())]);
        let tools = ["lint-tool".to_owned(), "other-tool".to_owned()];
        let findings = vec![
            finding("other-tool", Severity::Error, None),
            ranged("src/a.py", 0, 1),
        ];
        let text = render_sarif(&tools, &findings, &snapshots, false).expect("render");
        let typed: Sarif = serde_json::from_str(&text).expect("typed SARIF");
        assert_eq!(typed.version, serde_json::Value::String("2.1.0".to_owned()));
        assert_eq!(typed.runs.len(), 2);
        assert_eq!(typed.runs[0].tool.driver.name, "lint-tool");
        assert_eq!(typed.runs[0].results.as_ref().expect("results").len(), 1);
        assert!(typed.runs[0].invocations.is_some());
    }

    #[test]
    fn sarif_empty_and_multi_run_fixtures_are_schema_valid() {
        let empty = render_sarif(&["lint-tool".to_owned()], &[], &BTreeMap::new(), false)
            .expect("empty render");
        let typed_empty: Sarif = serde_json::from_str(&empty).expect("typed empty");
        assert_eq!(typed_empty.runs.len(), 1);
        assert!(typed_empty.runs[0]
            .results
            .as_ref()
            .expect("results")
            .is_empty());

        let snapshots = BTreeMap::new();
        let tools = ["b-tool".to_owned(), "a-tool".to_owned()];
        let findings = vec![
            finding("b-tool", Severity::Error, None),
            finding("a-tool", Severity::Info, None),
        ];
        let multi = render_sarif(&tools, &findings, &snapshots, true).expect("multi render");
        let typed_multi: Sarif = serde_json::from_str(&multi).expect("typed multi");
        assert_eq!(typed_multi.runs.len(), 2);
        assert_eq!(typed_multi.runs[0].tool.driver.name, "a-tool");
        assert_eq!(typed_multi.runs[1].tool.driver.name, "b-tool");
        for run in &typed_multi.runs {
            assert_eq!(run.results.as_ref().expect("results").len(), 1);
            assert!(run.invocations.is_none());
        }
    }

    #[test]
    fn sarif_marks_only_suppressed_results() {
        let snapshots = BTreeMap::from([("src/a.py".to_owned(), "x = 1\n".to_owned())]);
        let tools = ["lint-tool".to_owned()];
        let findings = vec![
            finding("lint-tool", Severity::Error, None),
            ranged("src/a.py", 0, 1),
        ];
        let plain = render_sarif(&tools, &findings, &snapshots, true).expect("plain");
        let unflagged =
            render_sarif_with_baseline(&tools, &findings, &[], &snapshots, true).expect("unflagged");
        assert_eq!(plain, unflagged);
        let marked =
            render_sarif_with_baseline(&tools, &findings, &[true, false], &snapshots, true)
                .expect("marked");
        let document: Value = serde_json::from_str(&marked).expect("valid JSON");
        let results = document["runs"][0]["results"].as_array().expect("results");
        assert_eq!(results.len(), 2);
        assert!(results[0].get("suppressions").is_none());
        assert_eq!(
            results[1]["suppressions"],
            json!([{"kind": "external", "justification": "accepted quality baseline"}])
        );
        let typed: Sarif = serde_json::from_str(&marked).expect("typed SARIF");
        assert_eq!(typed.runs.len(), 1);
        let short = render_sarif_with_baseline(&tools, &findings, &[true], &snapshots, true)
            .expect("short flags");
        assert_eq!(short, plain);
    }
}
