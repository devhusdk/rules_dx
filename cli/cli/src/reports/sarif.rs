use std::collections::{BTreeMap, BTreeSet};

use super::{finding_identity, ReportError};
use dx_fingerprint::baseline::finding_fingerprint;
use dx_output::{sort_diagnostics, DiagnosticEvent, Severity, Snapshot};
use line_index::{LineIndex, TextSize, WideEncoding};
use serde_json::Value;
use serde_sarif::sarif::{
    ArtifactChange, ArtifactContent, ArtifactLocation, Fix, Invocation, Location, Message,
    Notification, PhysicalLocation, Region, Replacement, ReportingDescriptor,
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
    snapshots: &BTreeMap<(Snapshot, String), String>,
) -> Result<Option<Location>, ReportError> {
    locate(
        finding.path.as_ref(),
        finding.range,
        finding.snapshot,
        snapshots,
    )
}

fn locate(
    path: Option<&String>,
    range: Option<(u64, u64)>,
    snapshot: Snapshot,
    snapshots: &BTreeMap<(Snapshot, String), String>,
) -> Result<Option<Location>, ReportError> {
    let Some(path) = path else {
        if range.is_some() {
            return Err(ReportError::RangeWithoutPath);
        }
        return Ok(None);
    };
    let artifact = ArtifactLocation::builder().uri(path.clone()).build();
    if let Some((start, end)) = range {
        if start > end {
            return Err(ReportError::InvertedRange);
        }
        let text = snapshots
            .get(&(snapshot, path.clone()))
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

fn related_locations(
    finding: &DiagnosticEvent,
    snapshots: &BTreeMap<(Snapshot, String), String>,
) -> Result<Vec<Location>, ReportError> {
    let mut out = Vec::with_capacity(finding.related.len());
    for (index, item) in finding.related.iter().enumerate() {
        let mut location = locate(item.path.as_ref(), item.range, finding.snapshot, snapshots)
            .or_else(|_| locate(item.path.as_ref(), None, finding.snapshot, snapshots))?
            .unwrap_or_else(|| Location::builder().build());
        location.id = Some(index as i64);
        let text = if item.role.is_empty() {
            item.message.clone()
        } else {
            format!("{}: {}", item.role, item.message)
        };
        location.message = Some(Message::builder().text(text).build());
        out.push(location);
    }
    Ok(out)
}

fn machine_fix(
    finding: &DiagnosticEvent,
    snapshots: &BTreeMap<(Snapshot, String), String>,
) -> Option<Fix> {
    let mut by_path: BTreeMap<&str, Vec<(u64, u64, &str)>> = BTreeMap::new();
    for item in &finding.related {
        let Some(replacement) = item.replacement.as_deref() else {
            continue;
        };
        if item.applicability.as_deref() != Some("MachineApplicable") {
            continue;
        }
        let (Some(path), Some((start, end))) = (item.path.as_deref(), item.range) else {
            return None;
        };
        by_path
            .entry(path)
            .or_default()
            .push((start, end, replacement));
    }
    if by_path.is_empty() {
        return None;
    }
    let mut changes = Vec::with_capacity(by_path.len());
    for (path, mut edits) in by_path {
        edits.sort_unstable();
        if edits
            .windows(2)
            .any(|pair| pair[1].0 < pair[0].1 || pair[1].0 == pair[0].0)
        {
            return None;
        }
        let mut replacements = Vec::with_capacity(edits.len());
        for (start, end, text) in edits {
            let location = locate(
                Some(&path.to_owned()),
                Some((start, end)),
                finding.snapshot,
                snapshots,
            )
            .ok()??;
            let region = location.physical_location?.region?;
            replacements.push(
                Replacement::builder()
                    .deleted_region(region)
                    .inserted_content(ArtifactContent::builder().text(text.to_owned()).build())
                    .build(),
            );
        }
        changes.push(
            ArtifactChange::builder()
                .artifact_location(ArtifactLocation::builder().uri(path.to_owned()).build())
                .replacements(replacements)
                .build(),
        );
    }
    Some(Fix::builder().artifact_changes(changes).build())
}

fn result(
    finding: &DiagnosticEvent,
    snapshots: &BTreeMap<(Snapshot, String), String>,
    rule_index: Option<i64>,
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
    let mut built = match (&finding.rule, locations) {
        (Some(rule), Some(locations)) => SarifResult::builder()
            .message(message)
            .level(level)
            .rule_id(rule.clone())
            .locations(locations)
            .build(),
        (Some(rule), None) => SarifResult::builder()
            .message(message)
            .level(level)
            .rule_id(rule.clone())
            .build(),
        (None, Some(locations)) => SarifResult::builder()
            .message(message)
            .level(level)
            .locations(locations)
            .build(),
        (None, None) => SarifResult::builder().message(message).level(level).build(),
    };
    if !finding.related.is_empty() {
        built.related_locations = Some(related_locations(finding, snapshots)?);
    }
    built.rule_index = rule_index;
    let bytes = finding
        .path
        .as_ref()
        .and_then(|path| snapshots.get(&(finding.snapshot, path.clone())))
        .map(String::as_bytes);
    if let Some(fingerprint) = finding_fingerprint(&finding_identity(finding, bytes)) {
        built.partial_fingerprints = Some(BTreeMap::from([("dx/v1".to_owned(), fingerprint)]));
    }
    if let Some(fix) = machine_fix(finding, snapshots) {
        built.fixes = Some(vec![fix]);
    }
    Ok(built)
}

pub fn render_sarif(
    tools: &[String],
    findings: &[DiagnosticEvent],
    snapshots: &BTreeMap<(Snapshot, String), String>,
    complete: bool,
) -> Result<String, ReportError> {
    render_sarif_with_baseline(tools, findings, snapshots, complete, &BTreeMap::new(), None)
}

pub struct BaselineReport {
    pub file: String,
    pub total: u64,
    pub new: u64,
    pub suppressed: u64,
}

pub fn render_sarif_with_baseline(
    tools: &[String],
    findings: &[DiagnosticEvent],
    snapshots: &BTreeMap<(Snapshot, String), String>,
    complete: bool,
    failures: &BTreeMap<String, Vec<String>>,
    baseline: Option<&BaselineReport>,
) -> Result<String, ReportError> {
    let rendered = render_inner(tools, findings, snapshots, complete, failures)?;
    let Some(baseline) = baseline else {
        return Ok(rendered);
    };
    let mut document: serde_json::Value =
        serde_json::from_str(&rendered).map_err(|_| ReportError::InvalidFinding {
            detail: "sarif render is not JSON",
        })?;
    let properties = serde_json::json!({
        "dxBaseline": {
            "file": baseline.file,
            "total": baseline.total,
            "new": baseline.new,
            "suppressed": baseline.suppressed,
        }
    });
    let runs = document
        .get_mut("runs")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or(ReportError::InvalidFinding {
            detail: "sarif render has no runs",
        })?;
    for run in runs.iter_mut() {
        run["properties"] = properties.clone();
    }
    dx_fingerprint::to_json(&document).map_err(|_| ReportError::InvalidFinding {
        detail: "sarif render failed to serialize",
    })
}

fn render_inner(
    tools: &[String],
    findings: &[DiagnosticEvent],
    snapshots: &BTreeMap<(Snapshot, String), String>,
    complete: bool,
    failures: &BTreeMap<String, Vec<String>>,
) -> Result<String, ReportError> {
    let ordered: BTreeSet<&str> = tools.iter().map(String::as_str).collect();
    let mut working = findings.to_vec();
    sort_diagnostics(&mut working);
    let mut by_tool: BTreeMap<&str, Vec<&DiagnosticEvent>> = BTreeMap::new();
    for finding in &working {
        check_shape(finding)?;
        if !ordered.contains(finding.tool.as_str()) {
            return Err(ReportError::UnknownTool {
                tool: finding.tool.clone(),
            });
        }
        by_tool
            .entry(finding.tool.as_str())
            .or_default()
            .push(finding);
    }
    let mut runs = Vec::with_capacity(ordered.len());
    for tool in ordered {
        let tool_findings = by_tool.get(tool).cloned().unwrap_or_default();
        let rules: Vec<&str> = tool_findings
            .iter()
            .filter_map(|finding| finding.rule.as_deref())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut results = Vec::with_capacity(tool_findings.len());
        for finding in tool_findings {
            let index = finding
                .rule
                .as_deref()
                .and_then(|rule| rules.binary_search(&rule).ok())
                .map(|position| position as i64);
            results.push(result(finding, snapshots, index)?);
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
        let failed = failures.get(tool);
        if complete && failed.is_none() {
            let mut run = Run::builder().tool(tool_value).results(results).build();
            run.column_kind = Some(Value::String("unicodeCodePoints".to_owned()));
            runs.push(run);
        } else {
            let mut invocation = Invocation::builder().execution_successful(false).build();
            invocation.tool_execution_notifications = failed.map(|messages| {
                messages
                    .iter()
                    .map(|text| {
                        Notification::builder()
                            .message(Message::builder().text(text.clone()).build())
                            .level(serde_json::Value::String("error".to_owned()))
                            .build()
                    })
                    .collect()
            });
            let mut run = Run::builder()
                .tool(tool_value)
                .results(results)
                .invocations(vec![invocation])
                .build();
            run.column_kind = Some(Value::String("unicodeCodePoints".to_owned()));
            runs.push(run);
        }
    }
    let document = Sarif::builder()
        .schema("https://json.schemastore.org/sarif-2.1.0.json".to_owned())
        .runs(runs)
        .version(serde_json::Value::String("2.1.0".to_owned()))
        .build();
    Ok(dx_fingerprint::to_json(&document)?)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use serde_json::{json, Value};

    fn finding(tool: &str, severity: Severity, path: Option<&str>) -> DiagnosticEvent {
        DiagnosticEvent {
            rule: Some(format!("{tool}/rule")),
            path: path.map(str::to_owned),
            ..DiagnosticEvent::new(severity, tool, format!("{tool} finding"))
        }
    }

    fn ranged(path: &str, start: u64, end: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            rule: Some("lint-tool/rule".to_owned()),
            path: Some(path.to_owned()),
            range: Some((start, end)),
            ..DiagnosticEvent::new(Severity::Warning, "lint-tool", "ranged finding")
        }
    }

    fn fix_finding(applicability: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            related: vec![dx_output::RelatedEvent {
                role: "suggestion".to_owned(),
                message: "use y".to_owned(),
                path: Some("src/a.py".to_owned()),
                range: Some((0, 1)),
                replacement: Some("y".to_owned()),
                applicability: Some(applicability.to_owned()),
            }],
            ..ranged("src/a.py", 0, 1)
        }
    }

    fn render_one(
        finding: DiagnosticEvent,
        snapshots: &BTreeMap<(Snapshot, String), String>,
    ) -> Value {
        let tools = [finding.tool.clone()];
        let text = render_sarif(&tools, &[finding], snapshots, true).expect("render");
        serde_json::from_str::<Value>(&text).expect("json")["runs"][0].clone()
    }

    #[test]
    fn sarif_fixes_only_come_from_machine_applicable_items() {
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "x = 1\n".to_owned(),
        )]);
        let run = render_one(fix_finding("MachineApplicable"), &snapshots);
        assert_eq!(
            run["results"][0]["fixes"],
            json!([{"artifactChanges": [{
                "artifactLocation": {"uri": "src/a.py"},
                "replacements": [{
                    "deletedRegion": {
                        "startLine": 1, "startColumn": 1, "endLine": 1, "endColumn": 2
                    },
                    "insertedContent": {"text": "y"}
                }]
            }]}])
        );
        let run = render_one(fix_finding("MaybeIncorrect"), &snapshots);
        assert!(run["results"][0].get("fixes").is_none());
    }

    #[test]
    fn sarif_drops_the_whole_fix_when_replacements_overlap_or_do_not_resolve() {
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "x = 1\n".to_owned(),
        )]);
        let mut overlapping = fix_finding("MachineApplicable");
        let mut second = overlapping.related[0].clone();
        second.range = Some((0, 3));
        overlapping.related.push(second);
        let run = render_one(overlapping, &snapshots);
        assert!(run["results"][0].get("fixes").is_none());
        let mut unresolved = fix_finding("MachineApplicable");
        unresolved.related[0].range = Some((0, 99));
        let run = render_one(unresolved, &snapshots);
        assert!(run["results"][0].get("fixes").is_none());
    }

    #[test]
    fn sarif_fingerprints_follow_the_baseline_identity() {
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "x = 1\n".to_owned(),
        )]);
        let origin = dx_output::Origin {
            producer: "p".to_owned(),
            target: "//a:b".to_owned(),
            configuration: None,
        };
        let pathless = DiagnosticEvent {
            origins: vec![origin],
            ..finding("lint-tool", Severity::Error, None)
        };
        let pathed = DiagnosticEvent {
            path: Some("src/a.py".to_owned()),
            ..finding("lint-tool", Severity::Error, None)
        };
        for diagnostic in [pathless, pathed] {
            let bytes = diagnostic
                .path
                .as_ref()
                .and_then(|path| snapshots.get(&(diagnostic.snapshot, path.clone())))
                .map(String::as_bytes);
            let want = finding_fingerprint(&finding_identity(&diagnostic, bytes)).expect("key");
            let run = render_one(diagnostic, &snapshots);
            assert_eq!(
                run["results"][0]["partialFingerprints"]["dx/v1"],
                json!(want)
            );
        }
        let bare = finding("lint-tool", Severity::Error, None);
        let run = render_one(bare, &snapshots);
        assert!(run["results"][0].get("partialFingerprints").is_none());
    }

    #[test]
    fn sarif_columns_count_code_points() {
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "\u{1f600}x = 1\n".to_owned(),
        )]);
        let run = render_one(ranged("src/a.py", 4, 5), &snapshots);
        assert_eq!(run["columnKind"], json!("unicodeCodePoints"));
        assert_eq!(
            run["results"][0]["locations"][0]["physicalLocation"]["region"]["startColumn"],
            json!(2)
        );
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
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "x = 1\nxx = 2\n".to_owned(),
        )]);
        let tools = ["lint-tool".to_owned(), "other-tool".to_owned()];
        let findings = vec![
            finding("other-tool", Severity::Error, None),
            ranged("src/a.py", 0, 5),
            DiagnosticEvent {
                path: Some("src/a.py".to_owned()),
                ..DiagnosticEvent::new(Severity::Info, "lint-tool", "note without rule")
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
        let index = lint_results[0]["ruleIndex"].as_u64().expect("ruleIndex") as usize;
        assert_eq!(rules[index]["id"], lint_results[0]["ruleId"]);
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
    fn sarif_marks_baseline_counts_on_every_run() {
        let findings = vec![finding("lint-tool", Severity::Warning, None)];
        let plain: Value = serde_json::from_str(
            &render_sarif(&["lint-tool".to_owned()], &findings, &BTreeMap::new(), true)
                .expect("render"),
        )
        .expect("valid JSON");
        assert!(plain["runs"][0].get("properties").is_none());
        let report = BaselineReport {
            file: "quality/baseline.json".to_owned(),
            total: 4,
            new: 1,
            suppressed: 3,
        };
        let marked: Value = serde_json::from_str(
            &render_sarif_with_baseline(
                &["lint-tool".to_owned()],
                &findings,
                &BTreeMap::new(),
                true,
                &BTreeMap::new(),
                Some(&report),
            )
            .expect("render"),
        )
        .expect("valid JSON");
        assert_eq!(
            marked["runs"][0]["properties"]["dxBaseline"],
            json!({
                "file": "quality/baseline.json",
                "total": 4,
                "new": 1,
                "suppressed": 3,
            })
        );
        assert_eq!(
            marked["runs"][0]["results"]
                .as_array()
                .expect("results")
                .len(),
            1,
            "suppressed findings stay visible in SARIF"
        );
    }

    #[test]
    fn sarif_names_a_failed_producer_without_findings() {
        let tools = ["fmt-tool".to_owned(), "lint-tool".to_owned()];
        let failures = BTreeMap::from([(
            "fmt-tool".to_owned(),
            vec!["//a:one did not converge: OSCILLATION after 2 round(s) of fmt-tool".to_owned()],
        )]);
        let document: Value = serde_json::from_str(
            &render_sarif_with_baseline(&tools, &[], &BTreeMap::new(), true, &failures, None)
                .expect("render"),
        )
        .expect("valid JSON");
        assert_eq!(
            document["runs"][0]["invocations"],
            json!([{
                "executionSuccessful": false,
                "toolExecutionNotifications": [{
                    "level": "error",
                    "message": {
                        "text": "//a:one did not converge: OSCILLATION after 2 round(s) of fmt-tool"
                    },
                }],
            }])
        );
        assert_eq!(document["runs"][0]["results"], json!([]));
        assert!(document["runs"][1].get("invocations").is_none());
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
                &BTreeMap::from([((Snapshot::Terminal, "src/a.py".to_owned()), "ab".to_owned())]),
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
                None,
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
                None,
            ),
            Err(ReportError::InvalidFinding {
                detail: "empty message"
            })
        );
    }

    #[test]
    fn sarif_output_parses_as_typed_sarif() {
        let snapshots = BTreeMap::from([(
            (Snapshot::Terminal, "src/a.py".to_owned()),
            "x = 1\n".to_owned(),
        )]);
        let tools = ["lint-tool".to_owned(), "other-tool".to_owned()];
        let findings = vec![
            finding("other-tool", Severity::Error, None),
            ranged("src/a.py", 0, 1),
            fix_finding("MachineApplicable"),
        ];
        let text = render_sarif(&tools, &findings, &snapshots, false).expect("render");
        let typed: Sarif = serde_json::from_str(&text).expect("typed SARIF");
        let results = typed.runs[0].results.as_ref().expect("results");
        assert!(results.iter().any(|result| result.fixes.is_some()));
        let first = &results[0];
        assert!(first.partial_fingerprints.is_some());
        assert!(first.rule_index.is_some());
        assert!(typed.runs[0].column_kind.is_some());
        assert_eq!(typed.version, serde_json::Value::String("2.1.0".to_owned()));
        assert_eq!(typed.runs.len(), 2);
        assert_eq!(typed.runs[0].tool.driver.name, "lint-tool");
        assert_eq!(typed.runs[0].results.as_ref().expect("results").len(), 2);
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
    fn related_context_renders_as_related_locations() {
        let mut event = ranged("src/a.rs", 0, 1);
        event.related = vec![
            dx_output::RelatedEvent {
                role: "label".to_owned(),
                message: "defined here".to_owned(),
                path: Some("src/b.rs".to_owned()),
                range: Some((1, 2)),
                replacement: None,
                applicability: None,
            },
            dx_output::RelatedEvent {
                role: "note".to_owned(),
                message: "no snapshot".to_owned(),
                path: Some("src/c.rs".to_owned()),
                range: Some((0, 1)),
                replacement: None,
                applicability: None,
            },
            dx_output::RelatedEvent {
                role: "help".to_owned(),
                message: "unlocated".to_owned(),
                path: None,
                range: None,
                replacement: None,
                applicability: None,
            },
        ];
        let mut snapshots = BTreeMap::new();
        snapshots.insert(
            (Snapshot::Terminal, "src/a.rs".to_owned()),
            "ab\n".to_owned(),
        );
        snapshots.insert(
            (Snapshot::Terminal, "src/b.rs".to_owned()),
            "x\r\n\u{e9}\n".to_owned(),
        );
        let document =
            render_sarif(&["lint-tool".to_owned()], &[event], &snapshots, true).expect("render");
        let value: Value = serde_json::from_str(&document).expect("json");
        let result = &value["runs"][0]["results"][0];
        assert_eq!(result["locations"].as_array().map(Vec::len), Some(1));
        let related = result["relatedLocations"].as_array().expect("related");
        assert_eq!(related.len(), 3);
        assert_eq!(related[0]["message"]["text"], json!("label: defined here"));
        assert_eq!(
            related[0]["physicalLocation"]["region"]["startLine"],
            json!(1)
        );
        assert!(related[1]["physicalLocation"]["region"].is_null());
        assert_eq!(
            related[1]["physicalLocation"]["artifactLocation"]["uri"],
            json!("src/c.rs")
        );
        assert!(related[2]["physicalLocation"].is_null());
        assert_eq!(related[2]["id"], json!(2));
    }
}
