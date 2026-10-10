use crate::findings::DiagnosticEvent;
use crate::lifecycle::base;
use crate::validation::{check_path, OutputError};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaselineError {
    #[error("baseline is not valid JSON: {detail}")]
    Json { detail: String },
    #[error("baseline needs object with version and entries")]
    Shape,
    #[error("baseline version {found}: want {BASELINE_VERSION}")]
    Version { found: String },
    #[error("baseline entry {index}: unknown field {field:?}")]
    UnknownField { index: usize, field: String },
    #[error("baseline entry {index}: tool must be a non-empty string")]
    EmptyTool { index: usize },
    #[error("baseline entry {index}: message must be a non-empty string")]
    EmptyMessage { index: usize },
    #[error("baseline entry {index}: path is not a workspace-relative file")]
    BadPath { index: usize },
    #[error("baseline entry {index}: rule must be a non-empty string or null")]
    BadRule { index: usize },
    #[error("baseline entry {index}: context must be 64 hex characters")]
    BadContext { index: usize },
}

impl From<BaselineError> for OutputError {
    fn from(error: BaselineError) -> Self {
        OutputError::Io(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaselineEntry {
    pub tool: String,
    pub rule: Option<String>,
    pub path: String,
    pub message: String,
    pub context: [u8; 32],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselineFile {
    pub entries: Vec<BaselineEntry>,
}

fn entry_string(
    map: &serde_json::Map<String, Value>,
    index: usize,
    key: &str,
) -> Result<Option<String>, BaselineError> {
    match map.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Null) if key == "rule" => Ok(None),
        Some(_) => Err(match key {
            "tool" => BaselineError::EmptyTool { index },
            "message" => BaselineError::EmptyMessage { index },
            "path" => BaselineError::BadPath { index },
            _ => BaselineError::BadRule { index },
        }),
    }
}

fn parse_entry(
    index: usize,
    value: &Value,
) -> Result<BaselineEntry, BaselineError> {
    let Value::Object(map) = value else {
        return Err(BaselineError::Shape);
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "tool" | "rule" | "path" | "message" | "context") {
            return Err(BaselineError::UnknownField {
                index,
                field: key.clone(),
            });
        }
    }
    let tool = entry_string(map, index, "tool")?
        .filter(|text| !text.is_empty())
        .ok_or(BaselineError::EmptyTool { index })?;
    let message = entry_string(map, index, "message")?
        .filter(|text| !text.is_empty())
        .ok_or(BaselineError::EmptyMessage { index })?;
    let path = entry_string(map, index, "path")?
        .filter(|text| !text.is_empty())
        .ok_or(BaselineError::BadPath { index })?;
    if check_path(&path).is_err() {
        return Err(BaselineError::BadPath { index });
    }
    let rule = match entry_string(map, index, "rule")? {
        None => None,
        Some(text) if text.is_empty() => return Err(BaselineError::BadRule { index }),
        Some(text) => Some(text),
    };
    let context = match map.get("context") {
        Some(Value::String(text)) => dx_digest::parse_hex(text)
            .map_err(|_| BaselineError::BadContext { index })?,
        _ => return Err(BaselineError::BadContext { index }),
    };
    Ok(BaselineEntry {
        tool,
        rule,
        path,
        message: normalize_message(&message),
        context,
    })
}

pub fn parse_baseline(bytes: &[u8]) -> Result<BaselineFile, BaselineError> {
    let text = std::str::from_utf8(bytes).map_err(|_| BaselineError::Shape)?;
    let value: Value =
        serde_json::from_str(text).map_err(|error| BaselineError::Json {
            detail: error.to_string(),
        })?;
    let Value::Object(map) = &value else {
        return Err(BaselineError::Shape);
    };
    if map.len() != 2 || !map.contains_key("version") || !map.contains_key("entries") {
        return Err(BaselineError::Shape);
    }
    match map.get("version") {
        Some(Value::Number(version)) if version.as_u64() == Some(BASELINE_VERSION as u64) => {}
        other => {
            return Err(BaselineError::Version {
                found: other
                    .map(|version| version.to_string())
                    .unwrap_or_else(|| "missing".to_owned()),
            });
        }
    }
    let Some(Value::Array(entries)) = map.get("entries") else {
        return Err(BaselineError::Shape);
    };
    let mut parsed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        parsed.push(parse_entry(index, entry)?);
    }
    Ok(BaselineFile { entries: parsed })
}

pub fn normalize_message(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn line_digest(source: &[u8], start: u64) -> Option<[u8; 32]> {
    let start = usize::try_from(start).ok()?;
    if start > source.len() {
        return None;
    }
    let line_start = source[..start]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|position| position + 1)
        .unwrap_or(0);
    let line_end = source[line_start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|position| line_start + position)
        .unwrap_or(source.len());
    Some(dx_digest::blake3(&source[line_start..line_end]))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fingerprint {
    pub tool: String,
    pub rule: Option<String>,
    pub path: String,
    pub message: String,
    pub context: [u8; 32],
}

pub fn fingerprint(
    diagnostic: &DiagnosticEvent,
    source: Option<&[u8]>,
) -> Option<Fingerprint> {
    let path = diagnostic.path.clone()?;
    let bytes = source?;
    let context = match diagnostic.range {
        Some((start, _)) => line_digest(bytes, start)?,
        None => dx_digest::blake3(bytes),
    };
    Some(Fingerprint {
        tool: diagnostic.tool.clone(),
        rule: diagnostic.rule.clone(),
        path,
        message: normalize_message(&diagnostic.message),
        context,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselineMatch {
    pub suppressed: Vec<bool>,
    pub stale: Vec<usize>,
}

fn entry_key(finding: &Fingerprint) -> BaselineEntry {
    BaselineEntry {
        tool: finding.tool.clone(),
        rule: finding.rule.clone(),
        path: finding.path.clone(),
        message: finding.message.clone(),
        context: finding.context,
    }
}

pub fn match_baseline(
    baseline: &BaselineFile,
    findings: &[Fingerprint],
    coverage: &BTreeSet<(String, String)>,
) -> BaselineMatch {
    let mut remaining: BTreeMap<BaselineEntry, usize> = BTreeMap::new();
    for entry in &baseline.entries {
        *remaining.entry(entry.clone()).or_default() += 1;
    }
    let mut suppressed = vec![false; findings.len()];
    let mut consumed: BTreeMap<BaselineEntry, usize> = BTreeMap::new();
    for (index, finding) in findings.iter().enumerate() {
        let key = entry_key(finding);
        let left = remaining.get(&key).copied().unwrap_or(0);
        if left > 0 {
            remaining.insert(key.clone(), left - 1);
            *consumed.entry(key).or_default() += 1;
            suppressed[index] = true;
        }
    }
    let mut seen: BTreeMap<BaselineEntry, usize> = BTreeMap::new();
    let mut stale = Vec::new();
    for (index, entry) in baseline.entries.iter().enumerate() {
        let position = seen.entry(entry.clone()).or_default();
        *position += 1;
        let used = consumed.get(entry).copied().unwrap_or(0);
        if *position > used
            && coverage.contains(&(entry.tool.clone(), entry.path.clone()))
        {
            stale.push(index);
        }
    }
    BaselineMatch { suppressed, stale }
}

pub fn refresh_entries(
    baseline: &BaselineFile,
    findings: &[Fingerprint],
    coverage: &BTreeSet<(String, String)>,
) -> Vec<BaselineEntry> {
    let mut kept: Vec<BaselineEntry> = baseline
        .entries
        .iter()
        .filter(|entry| !coverage.contains(&(entry.tool.clone(), entry.path.clone())))
        .cloned()
        .collect();
    let mut fresh: Vec<BaselineEntry> = findings
        .iter()
        .filter(|finding| {
            coverage.contains(&(finding.tool.clone(), finding.path.clone()))
        })
        .map(|finding| BaselineEntry {
            tool: finding.tool.clone(),
            rule: finding.rule.clone(),
            path: finding.path.clone(),
            message: finding.message.clone(),
            context: finding.context,
        })
        .collect();
    fresh.sort();
    kept.extend(fresh);
    kept
}

pub fn render_baseline(entries: &[BaselineEntry]) -> String {
    let items: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let mut map = serde_json::Map::new();
            map.insert(
                "tool".to_owned(),
                Value::String(entry.tool.clone()),
            );
            map.insert(
                "rule".to_owned(),
                entry
                    .rule
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            map.insert(
                "path".to_owned(),
                Value::String(entry.path.clone()),
            );
            map.insert(
                "message".to_owned(),
                Value::String(entry.message.clone()),
            );
            map.insert(
                "context".to_owned(),
                Value::String(dx_digest::to_hex(&entry.context)),
            );
            Value::Object(map)
        })
        .collect();
    let mut document = serde_json::Map::new();
    document.insert(
        "version".to_owned(),
        Value::Number(serde_json::Number::from(BASELINE_VERSION)),
    );
    document.insert("entries".to_owned(), Value::Array(items));
    serde_json::to_string_pretty(&Value::Object(document)).unwrap_or_default() + "\n"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineCounts {
    pub total: u64,
    pub suppressed: u64,
    pub stale: u64,
}

pub fn baseline_event(
    path: &str,
    counts: &BaselineCounts,
    coverage_complete: bool,
) -> Result<Value, OutputError> {
    if path.is_empty() {
        return Err(OutputError::EmptyField { field: "path" });
    }
    let mut map = base("baseline");
    map.insert("path".to_owned(), Value::String(path.to_owned()));
    map.insert(
        "total".to_owned(),
        Value::Number(serde_json::Number::from(counts.total)),
    );
    map.insert(
        "suppressed".to_owned(),
        Value::Number(serde_json::Number::from(counts.suppressed)),
    );
    map.insert(
        "stale".to_owned(),
        Value::Number(serde_json::Number::from(counts.stale)),
    );
    map.insert(
        "coverage_complete".to_owned(),
        Value::Bool(coverage_complete),
    );
    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::findings::Snapshot;
    use crate::severity::Severity;

    const SHELL_SOURCE: &[u8] = b"#!/bin/sh\necho $hello\n";
    const SC2086_MESSAGE: &str = "Double quote to prevent globbing and word splitting.";
    const SC2154_MESSAGE: &str = "hello is referenced but not assigned.";

    fn shell_diagnostic(rule: &str, message: &str, severity: Severity) -> DiagnosticEvent {
        DiagnosticEvent {
            severity,
            tool: "shellcheck".to_owned(),
            message: message.to_owned(),
            rule: Some(rule.to_owned()),
            path: Some("scripts/hello.sh".to_owned()),
            range: Some((15, 21)),
            snapshot: Snapshot::Initial,
            fixable: false,
            resolution: None,
        }
    }

    fn shell_fingerprint(rule: &str, message: &str) -> Fingerprint {
        fingerprint(
            &shell_diagnostic(rule, message, Severity::Warning),
            Some(SHELL_SOURCE),
        )
        .expect("shell finding fingerprints")
    }

    fn shell_entry(rule: &str, message: &str) -> BaselineEntry {
        let print = shell_fingerprint(rule, message);
        BaselineEntry {
            tool: print.tool,
            rule: print.rule,
            path: print.path,
            message: print.message,
            context: print.context,
        }
    }

    fn coverage_of(entries: &[BaselineEntry]) -> BTreeSet<(String, String)> {
        entries
            .iter()
            .map(|entry| (entry.tool.clone(), entry.path.clone()))
            .collect()
    }

    #[test]
    fn messages_normalize_whitespace() {
        assert_eq!(normalize_message("  a\tb\n c  "), "a b c");
        assert_eq!(normalize_message("single"), "single");
        assert_eq!(normalize_message(""), "");
    }

    #[test]
    fn measured_shellcheck_findings_fingerprint() {
        let finding = shell_fingerprint("SC2086", SC2086_MESSAGE);
        assert_eq!(finding.tool, "shellcheck");
        assert_eq!(finding.rule, Some("SC2086".to_owned()));
        assert_eq!(finding.path, "scripts/hello.sh");
        assert_eq!(finding.message, SC2086_MESSAGE);
        assert_eq!(
            finding.context,
            dx_digest::blake3(b"echo $hello"),
            "context is the finding line, not its byte offsets"
        );
    }

    #[test]
    fn moved_lines_keep_their_fingerprint() {
        let moved: &[u8] = b"#!/bin/sh\n# a comment\necho $hello\n";
        let mut relocated = shell_diagnostic("SC2086", SC2086_MESSAGE, Severity::Warning);
        relocated.range = Some((27, 33));
        let print =
            fingerprint(&relocated, Some(moved)).expect("moved finding fingerprints");
        assert_eq!(
            print.context,
            dx_digest::blake3(b"echo $hello"),
            "offsets moved but the line did not"
        );
        let edited: &[u8] = b"#!/bin/sh\necho \"$hello\"\n";
        let changed = fingerprint(
            &shell_diagnostic("SC2086", SC2086_MESSAGE, Severity::Warning),
            Some(edited),
        )
        .expect("edited finding fingerprints");
        assert_ne!(
            print.context, changed.context,
            "changed context must not match the baselined line"
        );
    }

    #[test]
    fn pathless_findings_and_missing_sources_never_fingerprint() {
        let mut pathless = shell_diagnostic("SC2086", SC2086_MESSAGE, Severity::Warning);
        pathless.path = None;
        pathless.range = None;
        assert!(fingerprint(&pathless, Some(SHELL_SOURCE)).is_none());
        let ranged = shell_diagnostic("SC2086", SC2086_MESSAGE, Severity::Warning);
        assert!(fingerprint(&ranged, None).is_none());
    }

    #[test]
    fn rangeless_findings_digest_the_whole_file() {
        let mut file_level = shell_diagnostic("SC2086", SC2086_MESSAGE, Severity::Warning);
        file_level.range = None;
        let print =
            fingerprint(&file_level, Some(SHELL_SOURCE)).expect("file finding fingerprints");
        assert_eq!(print.context, dx_digest::blake3(SHELL_SOURCE));
    }

    #[test]
    fn render_round_trips_through_the_parser() {
        let entries = vec![
            shell_entry("SC2086", SC2086_MESSAGE),
            shell_entry("SC2154", SC2154_MESSAGE),
        ];
        let rendered = render_baseline(&entries);
        let parsed = parse_baseline(rendered.as_bytes()).expect("rendered parses");
        assert_eq!(parsed.entries, entries);
    }

    #[test]
    fn parser_rejects_malformed_documents() {
        for document in [
            "not json",
            "[1, 2]",
            "{}",
            "{\"version\": 1}",
            "{\"entries\": []}",
            "{\"version\": 1, \"entries\": [], \"extra\": true}",
            "{\"version\": 0, \"entries\": []}",
            "{\"version\": 2, \"entries\": []}",
            "{\"version\": \"1\", \"entries\": []}",
            "{\"version\": 1, \"entries\": {}}",
            "{\"version\": 1, \"entries\": [42]}",
        ] {
            assert!(
                parse_baseline(document.as_bytes()).is_err(),
                "document parses: {document}"
            );
        }
        assert_eq!(
            parse_baseline("{\"version\": 2, \"entries\": []}".as_bytes()),
            Err(BaselineError::Version {
                found: "2".to_owned()
            })
        );
    }

    fn entry_document(entry: &str) -> String {
        format!("{{\"version\": 1, \"entries\": [{entry}]}}")
    }

    #[test]
    fn parser_rejects_malformed_entries() {
        let good = "\"tool\": \"shellcheck\", \"path\": \"scripts/hello.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\"";
        parse_baseline(entry_document(&format!("{{{good}}}")).as_bytes()).expect("good parses");
        for (name, entry) in [
            ("empty tool", "\"tool\": \"\", \"path\": \"a.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("missing tool", "\"path\": \"a.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("empty message", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("missing path", "\"tool\": \"t\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("absolute path", "\"tool\": \"t\", \"path\": \"/a.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("dotdot path", "\"tool\": \"t\", \"path\": \"a/../b.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("empty rule", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\", \"rule\": \"\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\""),
            ("short context", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\", \"context\": \"00\""),
            ("nonhex context", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\", \"context\": \"zz00000000000000000000000000000000000000000000000000000000000000\""),
            ("missing context", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\""),
            ("unknown field", "\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\", \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\", \"mesage\": \"m\""),
        ] {
            assert!(
                parse_baseline(entry_document(&format!("{{{entry}}}")).as_bytes()).is_err(),
                "{name} parses"
            );
        }
        let nulled = parse_baseline(
            entry_document("{\"tool\": \"t\", \"path\": \"a.sh\", \"message\": \"m\", \"rule\": null, \"context\": \"0000000000000000000000000000000000000000000000000000000000000000\"}")
                .as_bytes(),
        )
        .expect("null rule parses");
        assert_eq!(nulled.entries[0].rule, None);
    }

    #[test]
    fn identical_findings_match_and_new_findings_do_not() {
        let baseline = BaselineFile {
            entries: vec![shell_entry("SC2086", SC2086_MESSAGE)],
        };
        let findings = vec![
            shell_fingerprint("SC2086", SC2086_MESSAGE),
            shell_fingerprint("SC2154", SC2154_MESSAGE),
        ];
        let coverage = coverage_of(&baseline.entries);
        let matched = match_baseline(&baseline, &findings, &coverage);
        assert_eq!(matched.suppressed, vec![true, false]);
        assert!(matched.stale.is_empty());
    }

    #[test]
    fn duplicate_findings_need_duplicate_entries() {
        let baseline = BaselineFile {
            entries: vec![shell_entry("SC2086", SC2086_MESSAGE)],
        };
        let findings = vec![
            shell_fingerprint("SC2086", SC2086_MESSAGE),
            shell_fingerprint("SC2086", SC2086_MESSAGE),
        ];
        let coverage = coverage_of(&baseline.entries);
        let matched = match_baseline(&baseline, &findings, &coverage);
        assert_eq!(matched.suppressed, vec![true, false]);
        let doubled = BaselineFile {
            entries: vec![
                shell_entry("SC2086", SC2086_MESSAGE),
                shell_entry("SC2086", SC2086_MESSAGE),
            ],
        };
        let matched = match_baseline(&doubled, &findings, &coverage);
        assert_eq!(matched.suppressed, vec![true, true]);
        assert!(matched.stale.is_empty());
    }

    #[test]
    fn renames_rules_and_messages_match_nothing() {
        let baseline = BaselineFile {
            entries: vec![shell_entry("SC2086", SC2086_MESSAGE)],
        };
        let coverage = coverage_of(&baseline.entries);
        let renamed = Fingerprint {
            path: "scripts/bye.sh".to_owned(),
            ..shell_fingerprint("SC2086", SC2086_MESSAGE)
        };
        let moved_coverage: BTreeSet<(String, String)> =
            [("shellcheck".to_owned(), "scripts/bye.sh".to_owned())]
                .into_iter()
                .collect();
        let matched = match_baseline(&baseline, std::slice::from_ref(&renamed), &moved_coverage);
        assert_eq!(matched.suppressed, vec![false]);
        assert!(
            matched.stale.is_empty(),
            "the old path left the analyzed scope, so it stays unevaluated"
        );
        let reruled = shell_fingerprint("SC2087", SC2086_MESSAGE);
        let matched = match_baseline(&baseline, std::slice::from_ref(&reruled), &coverage);
        assert_eq!(matched.suppressed, vec![false]);
        assert_eq!(
            matched.stale,
            vec![0],
            "same tool and path still analyzed: the entry is stale"
        );
        let reworded = shell_fingerprint("SC2086", "Double quote everything.");
        let matched = match_baseline(&baseline, std::slice::from_ref(&reworded), &coverage);
        assert_eq!(matched.suppressed, vec![false]);
        assert_eq!(matched.stale, vec![0]);
    }

    #[test]
    fn fixed_findings_leave_stale_entries_in_scope() {
        let baseline = BaselineFile {
            entries: vec![
                shell_entry("SC2086", SC2086_MESSAGE),
                shell_entry("SC2154", SC2154_MESSAGE),
            ],
        };
        let coverage = coverage_of(&baseline.entries);
        let findings = vec![shell_fingerprint("SC2154", SC2154_MESSAGE)];
        let matched = match_baseline(&baseline, &findings, &coverage);
        assert_eq!(matched.suppressed, vec![true]);
        assert_eq!(matched.stale, vec![0]);
    }

    #[test]
    fn refresh_keeps_out_of_scope_and_rewrites_in_scope() {
        let old = shell_entry("SC2086", SC2086_MESSAGE);
        let gone = BaselineEntry {
            path: "scripts/removed.sh".to_owned(),
            ..old.clone()
        };
        let baseline = BaselineFile {
            entries: vec![gone.clone(), old.clone()],
        };
        let coverage: BTreeSet<(String, String)> =
            [("shellcheck".to_owned(), "scripts/hello.sh".to_owned())]
                .into_iter()
                .collect();
        let findings = vec![shell_fingerprint("SC2154", SC2154_MESSAGE)];
        let refreshed = refresh_entries(&baseline, &findings, &coverage);
        assert_eq!(
            refreshed,
            vec![
                gone,
                BaselineEntry {
                    tool: "shellcheck".to_owned(),
                    rule: Some("SC2154".to_owned()),
                    path: "scripts/hello.sh".to_owned(),
                    message: SC2154_MESSAGE.to_owned(),
                    context: dx_digest::blake3(b"echo $hello"),
                }
            ]
        );
        let reparsed =
            parse_baseline(render_baseline(&refreshed).as_bytes()).expect("refresh parses");
        assert_eq!(reparsed.entries, refreshed);
    }

    #[test]
    fn baseline_event_carries_counts() {
        let event = baseline_event(
            "baselines/lint.json",
            &BaselineCounts {
                total: 3,
                suppressed: 1,
                stale: 1,
            },
            true,
        )
        .expect("event");
        assert_eq!(event["event"], Value::String("baseline".to_owned()));
        assert_eq!(event["path"], Value::String("baselines/lint.json".to_owned()));
        assert_eq!(event["total"], Value::Number(3.into()));
        assert_eq!(event["suppressed"], Value::Number(1.into()));
        assert_eq!(event["stale"], Value::Number(1.into()));
        assert_eq!(event["coverage_complete"], Value::Bool(true));
        assert!(baseline_event("", &BaselineCounts { total: 0, suppressed: 0, stale: 0 }, true).is_err());
    }
}
