use std::collections::{BTreeMap, BTreeSet};

pub const BASELINE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaselineError {
    #[error("malformed baseline: {detail}")]
    Json { detail: String },
    #[error("malformed baseline: unsupported schema {found}: want {BASELINE_SCHEMA_VERSION}")]
    UnsupportedSchema { found: u32 },
    #[error("malformed baseline entry {index}: want a non-empty tool identity")]
    EmptyTool { index: usize },
    #[error("malformed baseline entry {index}: invalid path {path:?}: {reason}")]
    BadPath {
        index: usize,
        path: String,
        reason: String,
    },
    #[error("malformed baseline entry {index}: invalid context digest: want 64 hex characters")]
    BadContext { index: usize },
    #[error("malformed baseline entry {index}: want a non-empty message")]
    EmptyMessage { index: usize },
    #[error("malformed baseline entry {index}: invalid count {found}: want at least 1")]
    BadCount { index: usize, found: u32 },
    #[error("malformed baseline entry {index}: duplicate of an earlier entry")]
    Duplicate { index: usize },
    #[error("cannot render baseline: {detail}")]
    Render { detail: String },
}

impl From<serde_json::Error> for BaselineError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json {
            detail: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub message: String,
    pub context: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineEntry {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub message: String,
    pub context: [u8; 32],
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselineFile {
    pub entries: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalyzedScope {
    pub tools: BTreeSet<String>,
    pub paths: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineMatch {
    pub suppressed: Vec<bool>,
    pub stale: Vec<BaselineEntry>,
}

pub fn normalize_message(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn line_context_digest(file_bytes: &[u8], start_byte: u64) -> Option<[u8; 32]> {
    if file_bytes.is_empty() {
        return None;
    }
    let start: usize = start_byte.try_into().ok()?;
    if start > file_bytes.len() {
        return None;
    }
    let mut line_start = start;
    while line_start > 0 && file_bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    let mut line_end = start;
    while line_end < file_bytes.len() && file_bytes[line_end] != b'\n' {
        line_end += 1;
    }
    let mut line = &file_bytes[line_start..line_end];
    while line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }
    Some(dx_digest::blake3(line))
}

pub fn finding_key(finding: &Finding) -> Option<String> {
    let context = finding.context?;
    Some(format!(
        "{}\0{}\0{}\0{}\0{}",
        finding.tool,
        finding.rule,
        finding.path,
        finding.message,
        dx_digest::to_hex(&context)
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryDoc {
    tool: String,
    rule: String,
    path: String,
    message: String,
    context: String,
    count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineDoc {
    schema_version: u32,
    entries: Vec<EntryDoc>,
}

fn check_entry(doc: EntryDoc, index: usize) -> Result<BaselineEntry, BaselineError> {
    if doc.tool.is_empty() {
        return Err(BaselineError::EmptyTool { index });
    }
    if !doc.path.is_empty() {
        if let Some(reason) = dx_path::reject_reason(&doc.path) {
            return Err(BaselineError::BadPath {
                index,
                path: doc.path,
                reason: reason.to_owned(),
            });
        }
    }
    let Some(context) = dx_digest::parse_hex(&doc.context).ok() else {
        return Err(BaselineError::BadContext { index });
    };
    let message = normalize_message(&doc.message);
    if message.is_empty() {
        return Err(BaselineError::EmptyMessage { index });
    }
    if doc.count == 0 {
        return Err(BaselineError::BadCount {
            index,
            found: doc.count,
        });
    }
    Ok(BaselineEntry {
        tool: doc.tool,
        rule: doc.rule,
        path: doc.path,
        message,
        context,
        count: doc.count,
    })
}

pub fn parse_baseline(bytes: &[u8]) -> Result<BaselineFile, BaselineError> {
    let doc: BaselineDoc = serde_json::from_slice(bytes).map_err(BaselineError::from)?;
    if doc.schema_version != BASELINE_SCHEMA_VERSION {
        return Err(BaselineError::UnsupportedSchema {
            found: doc.schema_version,
        });
    }
    let mut entries = Vec::with_capacity(doc.entries.len());
    let mut seen = BTreeSet::new();
    for (index, raw) in doc.entries.into_iter().enumerate() {
        let entry = check_entry(raw, index)?;
        let key = entry_key(&entry);
        if !seen.insert(key) {
            return Err(BaselineError::Duplicate { index });
        }
        entries.push(entry);
    }
    Ok(BaselineFile { entries })
}

fn entry_key(entry: &BaselineEntry) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}",
        entry.tool,
        entry.rule,
        entry.path,
        entry.message,
        dx_digest::to_hex(&entry.context)
    )
}

pub fn entry_in_scope(entry: &BaselineEntry, scope: &AnalyzedScope) -> bool {
    if !scope.tools.contains(&entry.tool) {
        return false;
    }
    entry.path.is_empty() || scope.paths.contains(&entry.path)
}

pub fn match_baseline(
    baseline: &BaselineFile,
    findings: &[Finding],
    scope: &AnalyzedScope,
    complete: bool,
) -> BaselineMatch {
    let mut remaining: BTreeMap<String, u32> = BTreeMap::new();
    for entry in &baseline.entries {
        *remaining.entry(entry_key(entry)).or_insert(0) += entry.count;
    }
    let mut suppressed = vec![false; findings.len()];
    for (index, finding) in findings.iter().enumerate() {
        let Some(key) = finding_key(finding) else {
            continue;
        };
        if let Some(left) = remaining.get_mut(&key) {
            if *left > 0 {
                *left -= 1;
                suppressed[index] = true;
            }
        }
    }
    let mut stale = Vec::new();
    if complete {
        for entry in &baseline.entries {
            let key = entry_key(entry);
            if remaining.get(&key).copied().unwrap_or(0) > 0 && entry_in_scope(entry, scope) {
                stale.push(entry.clone());
            }
        }
        stale.sort_by_key(entry_key);
    }
    BaselineMatch { suppressed, stale }
}

pub fn refresh_baseline(
    old: &BaselineFile,
    findings: &[Finding],
    scope: &AnalyzedScope,
) -> BaselineFile {
    let mut counts: BTreeMap<String, (BaselineEntry, u32)> = BTreeMap::new();
    for entry in &old.entries {
        if !entry_in_scope(entry, scope) {
            let key = entry_key(entry);
            counts.entry(key).or_insert_with(|| (entry.clone(), 0)).1 += entry.count;
        }
    }
    for finding in findings {
        let Some(context) = finding.context else {
            continue;
        };
        if !scope.tools.contains(&finding.tool) {
            continue;
        }
        if !finding.path.is_empty() && !scope.paths.contains(&finding.path) {
            continue;
        }
        let entry = BaselineEntry {
            tool: finding.tool.clone(),
            rule: finding.rule.clone(),
            path: finding.path.clone(),
            message: finding.message.clone(),
            context,
            count: 1,
        };
        let key = entry_key(&entry);
        counts.entry(key).or_insert_with(|| (entry, 0)).1 += 1;
    }
    let mut entries: Vec<BaselineEntry> = counts
        .into_values()
        .map(|(mut entry, count)| {
            entry.count = count;
            entry
        })
        .collect();
    entries.sort_by_key(entry_key);
    BaselineFile { entries }
}

pub fn render_baseline(baseline: &BaselineFile) -> Result<String, BaselineError> {
    let mut entries = baseline.entries.clone();
    entries.sort_by_key(entry_key);
    let rendered: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "tool": entry.tool,
                "rule": entry.rule,
                "path": entry.path,
                "message": entry.message,
                "context": dx_digest::to_hex(&entry.context),
                "count": entry.count,
            })
        })
        .collect();
    let document = serde_json::json!({
        "schema_version": BASELINE_SCHEMA_VERSION,
        "entries": rendered,
    });
    super::to_json_ascii_pretty(&document).map_err(|error| BaselineError::Render {
        detail: error.to_string(),
    })
}

pub fn describe_entry(entry: &BaselineEntry) -> String {
    if entry.rule.is_empty() {
        format!("{} [{}]: {}", entry.path, entry.tool, entry.message)
    } else {
        format!(
            "{} [{}/{}]: {}",
            entry.path, entry.tool, entry.rule, entry.message
        )
    }
}

pub fn describe_finding(finding: &Finding) -> String {
    if finding.rule.is_empty() {
        format!("{} [{}]: {}", finding.path, finding.tool, finding.message)
    } else {
        format!(
            "{} [{}/{}]: {}",
            finding.path, finding.tool, finding.rule, finding.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(tool: &str, rule: &str, path: &str, message: &str, context: &[u8]) -> Finding {
        Finding {
            tool: tool.to_owned(),
            rule: rule.to_owned(),
            path: path.to_owned(),
            message: normalize_message(message),
            context: Some(dx_digest::blake3(context)),
        }
    }

    fn scope(tools: &[&str], paths: &[&str]) -> AnalyzedScope {
        AnalyzedScope {
            tools: tools.iter().map(ToString::to_string).collect(),
            paths: paths.iter().map(ToString::to_string).collect(),
        }
    }

    fn baseline_json(entries: &str) -> Vec<u8> {
        format!("{{\"schema_version\": 1, \"entries\": [{entries}]}}").into_bytes()
    }

    fn entry_json(tool: &str, rule: &str, path: &str, message: &str, context: &[u8]) -> String {
        format!(
            "{{\"tool\": \"{tool}\", \"rule\": \"{rule}\", \"path\": \"{path}\", \"message\": \"{message}\", \"context\": \"{}\", \"count\": 1}}",
            dx_digest::to_hex(&dx_digest::blake3(context))
        )
    }

    #[test]
    fn messages_normalize_whitespace_runs() {
        assert_eq!(
            normalize_message("  unused   import\n next "),
            "unused import next"
        );
        assert_eq!(normalize_message("single"), "single");
        assert_eq!(normalize_message("   "), "");
    }

    #[test]
    fn line_context_ignores_offsets_but_not_content() {
        let body = b"first line\nsecond line\n";
        let first = line_context_digest(body, 3).expect("first line");
        let moved = line_context_digest(body, 15).expect("second line");
        assert_ne!(first, moved);
        let again = line_context_digest(body, 0).expect("line start");
        assert_eq!(first, again);
        assert!(line_context_digest(body, 100).is_none());
        assert!(line_context_digest(b"", 0).is_none());
    }

    #[test]
    fn line_context_strips_carriage_returns() {
        let unix = line_context_digest(b"a\r\nb", 0).expect("crlf");
        let plain = line_context_digest(b"a\nb", 0).expect("lf");
        assert_eq!(unix, plain);
    }

    #[test]
    fn valid_baseline_roundtrips_canonically() {
        let raw = baseline_json(&entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1"));
        let parsed = parse_baseline(&raw).expect("valid");
        assert_eq!(parsed.entries.len(), 1);
        let first = render_baseline(&parsed).expect("render");
        let second =
            render_baseline(&parse_baseline(first.as_bytes()).expect("reparse")).expect("rerender");
        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }

    #[test]
    fn malformed_documents_fail_with_named_causes() {
        assert!(matches!(
            parse_baseline(b"{not json"),
            Err(BaselineError::Json { .. })
        ));
        let unknown_field = baseline_json(
            "{\"tool\": \"ruff\", \"rule\": \"\", \"path\": \"a.py\", \"message\": \"m\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1, \"extra\": true}",
        );
        assert!(matches!(
            parse_baseline(&unknown_field),
            Err(BaselineError::Json { .. })
        ));
        let bad_schema = b"{\"schema_version\": 2, \"entries\": []}";
        assert_eq!(
            parse_baseline(bad_schema),
            Err(BaselineError::UnsupportedSchema { found: 2 })
        );
        let empty_tool = baseline_json(
            "{\"tool\": \"\", \"rule\": \"\", \"path\": \"a.py\", \"message\": \"m\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1}",
        );
        assert_eq!(
            parse_baseline(&empty_tool),
            Err(BaselineError::EmptyTool { index: 0 })
        );
        let absolute = baseline_json(
            "{\"tool\": \"ruff\", \"rule\": \"\", \"path\": \"/abs.py\", \"message\": \"m\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1}",
        );
        assert!(matches!(
            parse_baseline(&absolute),
            Err(BaselineError::BadPath { index: 0, .. })
        ));
        let short_context = baseline_json(
            "{\"tool\": \"ruff\", \"rule\": \"\", \"path\": \"a.py\", \"message\": \"m\", \"context\": \"abcd\", \"count\": 1}",
        );
        assert_eq!(
            parse_baseline(&short_context),
            Err(BaselineError::BadContext { index: 0 })
        );
        let blank_message = baseline_json(
            "{\"tool\": \"ruff\", \"rule\": \"\", \"path\": \"a.py\", \"message\": \"   \", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1}",
        );
        assert_eq!(
            parse_baseline(&blank_message),
            Err(BaselineError::EmptyMessage { index: 0 })
        );
        let zero_count = baseline_json(
            "{\"tool\": \"ruff\", \"rule\": \"\", \"path\": \"a.py\", \"message\": \"m\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 0}",
        );
        assert_eq!(
            parse_baseline(&zero_count),
            Err(BaselineError::BadCount { index: 0, found: 0 })
        );
        let once = entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1");
        let twice = baseline_json(&format!("{once},{once}"));
        assert_eq!(
            parse_baseline(&twice),
            Err(BaselineError::Duplicate { index: 1 })
        );
    }

    #[test]
    fn matching_suppresses_recorded_occurrences_only() {
        let raw = baseline_json(&entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1"));
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = scope(&["ruff"], &["src/a.py"]);
        let current = vec![
            finding("ruff", "F401", "src/a.py", "unused", b"x = 1"),
            finding("ruff", "F401", "src/a.py", "unused", b"x = 1"),
        ];
        let matched = match_baseline(&baseline, &current, &seen, true);
        assert_eq!(matched.suppressed, vec![true, false]);
        assert!(matched.stale.is_empty());
    }

    #[test]
    fn moved_lines_match_while_renames_and_rule_changes_do_not() {
        let raw = baseline_json(&entry_json(
            "ruff",
            "F401",
            "src/a.py",
            "unused",
            b"import os",
        ));
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = scope(&["ruff"], &["src/a.py", "src/b.py"]);
        let moved = finding("ruff", "F401", "src/a.py", "unused", b"import os");
        let matched = match_baseline(&baseline, &[moved], &seen, true);
        assert_eq!(matched.suppressed, vec![true]);
        let renamed = finding("ruff", "F401", "src/b.py", "unused", b"import os");
        let matched = match_baseline(&baseline, &[renamed], &seen, true);
        assert_eq!(matched.suppressed, vec![false]);
        assert_eq!(matched.stale.len(), 1);
        let changed_rule = finding("ruff", "F841", "src/a.py", "unused", b"import os");
        let matched = match_baseline(&baseline, &[changed_rule], &seen, true);
        assert_eq!(matched.suppressed, vec![false]);
        assert_eq!(matched.stale.len(), 1);
        let changed_text = finding("ruff", "F401", "src/a.py", "unused", b"import sys");
        let matched = match_baseline(&baseline, &[changed_text], &seen, true);
        assert_eq!(matched.suppressed, vec![false]);
        assert_eq!(matched.stale.len(), 1);
    }

    #[test]
    fn stale_entries_need_complete_in_scope_analysis() {
        let raw = baseline_json(&entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1"));
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = scope(&["ruff"], &["src/a.py"]);
        let fixed = match_baseline(&baseline, &[], &seen, true);
        assert_eq!(fixed.stale.len(), 1);
        let incomplete = match_baseline(&baseline, &[], &seen, false);
        assert!(incomplete.stale.is_empty());
        let other_tool = match_baseline(&baseline, &[], &scope(&["ty"], &["src/a.py"]), true);
        assert!(other_tool.stale.is_empty());
        let other_path = match_baseline(&baseline, &[], &scope(&["ruff"], &["src/b.py"]), true);
        assert!(other_path.stale.is_empty());
    }

    #[test]
    fn tool_level_findings_match_without_paths() {
        let raw = baseline_json(
            "{\"tool\": \"audit\", \"rule\": \"\", \"path\": \"\", \"message\": \"known advisory\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1}",
        );
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = scope(&["audit"], &["src/a.py"]);
        let digest = dx_digest::parse_hex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .expect("hex");
        let current = Finding {
            tool: "audit".to_owned(),
            rule: String::new(),
            path: String::new(),
            message: "known advisory".to_owned(),
            context: Some(digest),
        };
        let matched = match_baseline(&baseline, &[current], &seen, true);
        assert_eq!(matched.suppressed, vec![true]);
    }

    #[test]
    fn refresh_prunes_in_scope_and_preserves_the_rest() {
        let kept = entry_json("ty", "E", "src/keep.py", "old", b"keep");
        let stale = entry_json("ruff", "F401", "src/a.py", "fixed", b"gone");
        let old = parse_baseline(&baseline_json(&format!("{kept},{stale}"))).expect("valid");
        let seen = scope(&["ruff", "ty"], &["src/a.py"]);
        let current = vec![finding("ruff", "F401", "src/a.py", "fresh", b"x = 1")];
        let next = refresh_baseline(&old, &current, &seen);
        assert_eq!(next.entries.len(), 2);
        assert!(next.entries.iter().any(|entry| entry.path == "src/keep.py"));
        assert!(next.entries.iter().any(|entry| entry.message == "fresh"));
        assert!(next.entries.iter().all(|entry| entry.message != "fixed"));
        let rendered = render_baseline(&next).expect("render");
        let reparsed = parse_baseline(rendered.as_bytes()).expect("reparse");
        assert_eq!(reparsed, next);
    }

    #[test]
    fn context_free_findings_never_match_or_record() {
        let raw = baseline_json(&entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1"));
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = scope(&["ruff"], &["src/a.py"]);
        let blind = Finding {
            tool: "ruff".to_owned(),
            rule: "F401".to_owned(),
            path: "src/a.py".to_owned(),
            message: "unused".to_owned(),
            context: None,
        };
        let matched = match_baseline(&baseline, &[blind.clone()], &seen, true);
        assert_eq!(matched.suppressed, vec![false]);
        let next = refresh_baseline(&BaselineFile::default(), &[blind], &seen);
        assert!(next.entries.is_empty());
    }

    #[test]
    fn entries_describe_themselves_for_reports() {
        let entry = BaselineEntry {
            tool: "ruff".to_owned(),
            rule: "F401".to_owned(),
            path: "src/a.py".to_owned(),
            message: "unused".to_owned(),
            context: [0u8; 32],
            count: 2,
        };
        assert_eq!(describe_entry(&entry), "src/a.py [ruff/F401]: unused");
        let finding = Finding {
            tool: "audit".to_owned(),
            rule: String::new(),
            path: String::new(),
            message: "known".to_owned(),
            context: None,
        };
        assert_eq!(describe_finding(&finding), " [audit]: known");
    }
}
