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
    #[error("malformed baseline entry {index}: an entry without a path needs a target")]
    MissingTarget { index: usize },
    #[error("malformed baseline entry {index}: an entry with a path takes no target")]
    StrayTarget { index: usize },
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
    pub target: String,
    pub message: String,
    pub context: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineEntry {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub target: String,
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
    pub ran: BTreeSet<(String, String)>,
    pub pairs: BTreeSet<(String, String)>,
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
    Some(key_of(
        &finding.tool,
        &finding.rule,
        &finding.path,
        &finding.target,
        &finding.message,
        &context,
    ))
}

pub fn finding_fingerprint(finding: &Finding) -> Option<String> {
    finding_key(finding).map(|key| dx_digest::to_hex(&dx_digest::blake3(key.as_bytes())))
}

fn key_of(
    tool: &str,
    rule: &str,
    path: &str,
    target: &str,
    message: &str,
    context: &[u8; 32],
) -> String {
    let mut key = format!(
        "{tool}\0{rule}\0{path}\0{message}\0{}",
        dx_digest::to_hex(context)
    );
    if !target.is_empty() {
        key.push('\0');
        key.push_str(target);
    }
    key
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryDoc {
    tool: String,
    rule: String,
    path: String,
    #[serde(default)]
    target: String,
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
    if doc.path.is_empty() && doc.target.is_empty() {
        return Err(BaselineError::MissingTarget { index });
    }
    if !doc.path.is_empty() && !doc.target.is_empty() {
        return Err(BaselineError::StrayTarget { index });
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
        target: doc.target,
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
    key_of(
        &entry.tool,
        &entry.rule,
        &entry.path,
        &entry.target,
        &entry.message,
        &entry.context,
    )
}

pub fn entry_in_scope(entry: &BaselineEntry, scope: &AnalyzedScope) -> bool {
    if entry.path.is_empty() {
        return target_in_scope(scope, &entry.tool, &entry.target);
    }
    pair_in_scope(scope, &entry.tool, &entry.path)
}

fn target_in_scope(scope: &AnalyzedScope, tool: &str, target: &str) -> bool {
    scope.ran.contains(&(tool.to_owned(), target.to_owned()))
}

fn pair_in_scope(scope: &AnalyzedScope, tool: &str, path: &str) -> bool {
    scope.pairs.contains(&(tool.to_owned(), path.to_owned()))
}

type Blind = (String, String, String);

fn blind_pairs(findings: &[Finding]) -> BTreeSet<Blind> {
    findings
        .iter()
        .filter(|finding| finding.context.is_none())
        .map(|finding| {
            (
                finding.tool.clone(),
                finding.path.clone(),
                finding.target.clone(),
            )
        })
        .collect()
}

fn blind_key(entry: &BaselineEntry) -> Blind {
    (entry.tool.clone(), entry.path.clone(), entry.target.clone())
}

pub fn unidentified(findings: &[Finding]) -> Vec<String> {
    let mut found: Vec<String> = findings
        .iter()
        .filter(|finding| finding.context.is_none())
        .map(describe_finding)
        .collect();
    found.sort();
    found.dedup();
    found
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
        let blind = blind_pairs(findings);
        for entry in &baseline.entries {
            let key = entry_key(entry);
            if remaining.get(&key).copied().unwrap_or(0) > 0
                && entry_in_scope(entry, scope)
                && !blind.contains(&blind_key(entry))
            {
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
    let blind = blind_pairs(findings);
    for entry in &old.entries {
        if !entry_in_scope(entry, scope) || blind.contains(&blind_key(entry)) {
            let key = entry_key(entry);
            counts.entry(key).or_insert_with(|| (entry.clone(), 0)).1 += entry.count;
        }
    }
    for finding in findings {
        let Some(context) = finding.context else {
            continue;
        };
        let analyzed = if finding.path.is_empty() {
            target_in_scope(scope, &finding.tool, &finding.target)
        } else {
            pair_in_scope(scope, &finding.tool, &finding.path)
        };
        if !analyzed {
            continue;
        }
        let entry = BaselineEntry {
            tool: finding.tool.clone(),
            rule: finding.rule.clone(),
            path: finding.path.clone(),
            target: finding.target.clone(),
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
            let mut value = serde_json::json!({
                "tool": entry.tool,
                "rule": entry.rule,
                "path": entry.path,
                "message": entry.message,
                "context": dx_digest::to_hex(&entry.context),
                "count": entry.count,
            });
            if !entry.target.is_empty() {
                value["target"] = serde_json::Value::String(entry.target.clone());
            }
            value
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
    let place = if entry.path.is_empty() {
        entry.target.as_str()
    } else {
        entry.path.as_str()
    };
    if entry.rule.is_empty() {
        format!("{place} [{}]: {}", entry.tool, entry.message)
    } else {
        format!("{place} [{}/{}]: {}", entry.tool, entry.rule, entry.message)
    }
}

pub fn describe_finding(finding: &Finding) -> String {
    let place = if finding.path.is_empty() {
        finding.target.as_str()
    } else {
        finding.path.as_str()
    };
    if finding.rule.is_empty() {
        format!("{place} [{}]: {}", finding.tool, finding.message)
    } else {
        format!(
            "{place} [{}/{}]: {}",
            finding.tool, finding.rule, finding.message
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
            target: String::new(),
            message: normalize_message(message),
            context: Some(dx_digest::blake3(context)),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_follows_the_rule() {
        let a = finding("t", "r1", "a.rs", "m", b"x");
        let b = finding("t", "r2", "a.rs", "m", b"x");
        let first = finding_fingerprint(&a).expect("has context");
        assert_eq!(Some(first.clone()), finding_fingerprint(&a));
        assert_eq!(first.len(), 64);
        assert_ne!(Some(first), finding_fingerprint(&b));
        let mut bare = a;
        bare.context = None;
        assert_eq!(finding_fingerprint(&bare), None);
    }

    fn scope(tools: &[&str], paths: &[&str]) -> AnalyzedScope {
        AnalyzedScope {
            ran: BTreeSet::new(),
            pairs: tools
                .iter()
                .flat_map(|tool| {
                    paths
                        .iter()
                        .map(move |path| (tool.to_string(), path.to_string()))
                })
                .collect(),
        }
    }

    fn pair_scope(pairs: &[(&str, &str)]) -> AnalyzedScope {
        AnalyzedScope {
            ran: BTreeSet::new(),
            pairs: pairs
                .iter()
                .map(|(tool, path)| (tool.to_string(), path.to_string()))
                .collect(),
        }
    }

    fn ran_scope(ran: &[(&str, &str)]) -> AnalyzedScope {
        AnalyzedScope {
            ran: ran
                .iter()
                .map(|(tool, target)| (tool.to_string(), target.to_string()))
                .collect(),
            pairs: BTreeSet::new(),
        }
    }

    fn pathless_json(tool: &str, target: &str, message: &str, context: &[u8]) -> String {
        format!(
            "{{\"tool\": \"{tool}\", \"rule\": \"\", \"path\": \"\", \"target\": \"{target}\", \"message\": \"{message}\", \"context\": \"{}\", \"count\": 1}}",
            dx_digest::to_hex(&dx_digest::blake3(context))
        )
    }

    fn pathless(tool: &str, target: &str, message: &str, context: &[u8]) -> Finding {
        Finding {
            tool: tool.to_owned(),
            rule: String::new(),
            path: String::new(),
            target: target.to_owned(),
            message: message.to_owned(),
            context: Some(dx_digest::blake3(context)),
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
            "{\"tool\": \"audit\", \"rule\": \"\", \"path\": \"\", \"target\": \"//x\", \"message\": \"known advisory\", \"context\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", \"count\": 1}",
        );
        let baseline = parse_baseline(&raw).expect("valid");
        let seen = ran_scope(&[("audit", "//x")]);
        let digest = dx_digest::parse_hex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .expect("hex");
        let current = Finding {
            tool: "audit".to_owned(),
            rule: String::new(),
            path: String::new(),
            target: "//x".to_owned(),
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
            target: String::new(),
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
            target: String::new(),
            message: "unused".to_owned(),
            context: [0u8; 32],
            count: 2,
        };
        assert_eq!(describe_entry(&entry), "src/a.py [ruff/F401]: unused");
        let finding = Finding {
            tool: "audit".to_owned(),
            rule: String::new(),
            path: String::new(),
            target: "//x".to_owned(),
            message: "known".to_owned(),
            context: None,
        };
        assert_eq!(describe_finding(&finding), "//x [audit]: known");
    }

    #[test]
    fn coverage_is_per_tool_and_path_not_a_cross_product() {
        let on_a = entry_json("A", "R", "src/a.py", "m", b"a");
        let a_on_b = entry_json("A", "R", "src/b.py", "m", b"b");
        let b_on_b = entry_json("B", "R", "src/b.py", "m", b"b");
        let baseline =
            parse_baseline(&baseline_json(&format!("{on_a},{a_on_b},{b_on_b}"))).expect("valid");
        let seen = pair_scope(&[("A", "src/a.py"), ("B", "src/b.py")]);
        let matched = match_baseline(&baseline, &[], &seen, true);
        let stale: Vec<String> = matched.stale.iter().map(describe_entry).collect();
        assert_eq!(
            stale,
            vec![
                "src/a.py [A/R]: m".to_owned(),
                "src/b.py [B/R]: m".to_owned()
            ]
        );
        let next = refresh_baseline(&baseline, &[], &seen);
        assert_eq!(next.entries.len(), 1);
        assert_eq!(next.entries[0].tool, "A");
        assert_eq!(next.entries[0].path, "src/b.py");
    }

    #[test]
    fn refresh_skips_findings_outside_analyzed_pairs() {
        let seen = pair_scope(&[("A", "src/a.py")]);
        let outside = finding("A", "R", "src/b.py", "m", b"b");
        let next = refresh_baseline(&BaselineFile::default(), &[outside], &seen);
        assert!(next.entries.is_empty());
    }

    #[test]
    fn unidentified_findings_never_justify_deleting_entries() {
        let recorded = entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1");
        let baseline = parse_baseline(&baseline_json(&recorded)).expect("valid");
        let seen = scope(&["ruff"], &["src/a.py"]);
        let blind = Finding {
            tool: "ruff".to_owned(),
            rule: "F401".to_owned(),
            path: "src/a.py".to_owned(),
            target: String::new(),
            message: "unused".to_owned(),
            context: None,
        };
        let matched = match_baseline(&baseline, &[blind.clone()], &seen, true);
        assert!(matched.stale.is_empty());
        let next = refresh_baseline(&baseline, &[blind.clone()], &seen);
        assert_eq!(next, baseline);
        assert_eq!(unidentified(&[blind]), vec!["src/a.py [ruff/F401]: unused"]);
    }

    #[test]
    fn pathless_entry_survives_a_run_of_another_target() {
        let entry = pathless_json("A", "//x", "m", b"c");
        let baseline = parse_baseline(&baseline_json(&entry)).expect("valid");
        let only_y = ran_scope(&[("A", "//y")]);
        let matched = match_baseline(&baseline, &[], &only_y, true);
        assert!(matched.stale.is_empty());
        let next = refresh_baseline(&baseline, &[], &only_y);
        assert_eq!(next, baseline);
    }

    #[test]
    fn pathless_entry_is_stale_when_its_target_ran_clean() {
        let entry = pathless_json("A", "//x", "m", b"c");
        let baseline = parse_baseline(&baseline_json(&entry)).expect("valid");
        let ran = ran_scope(&[("A", "//x")]);
        let matched = match_baseline(&baseline, &[], &ran, true);
        assert_eq!(matched.stale.len(), 1);
        assert!(refresh_baseline(&baseline, &[], &ran).entries.is_empty());
    }

    #[test]
    fn pathless_findings_count_per_target() {
        let both = format!(
            "{},{}",
            pathless_json("A", "//x", "m", b"c"),
            pathless_json("A", "//y", "m", b"c")
        );
        let baseline = parse_baseline(&baseline_json(&both)).expect("valid");
        let ran = ran_scope(&[("A", "//x"), ("A", "//y")]);
        let current = [pathless("A", "//x", "m", b"c")];
        let matched = match_baseline(&baseline, &current, &ran, true);
        assert_eq!(matched.suppressed, vec![true]);
        assert_eq!(matched.stale.len(), 1);
        assert_eq!(matched.stale[0].target, "//y");
    }

    #[test]
    fn a_missing_or_stray_target_is_a_parse_error() {
        let missing = baseline_json(&entry_json("A", "", "", "m", b"c"));
        assert_eq!(
            parse_baseline(&missing),
            Err(BaselineError::MissingTarget { index: 0 })
        );
        let stray = baseline_json(&format!(
            "{{\"tool\": \"A\", \"rule\": \"\", \"path\": \"a.py\", \"target\": \"//x\", \"message\": \"m\", \"context\": \"{}\", \"count\": 1}}",
            dx_digest::to_hex(&dx_digest::blake3(b"c"))
        ));
        assert_eq!(
            parse_baseline(&stray),
            Err(BaselineError::StrayTarget { index: 0 })
        );
    }

    #[test]
    fn path_entries_render_without_a_target() {
        let raw = baseline_json(&entry_json("ruff", "F401", "src/a.py", "unused", b"x = 1"));
        let parsed = parse_baseline(&raw).expect("valid");
        let rendered = render_baseline(&parsed).expect("render");
        assert!(!rendered.contains("target"), "{rendered}");
        let entry = pathless_json("A", "//x", "m", b"c");
        let parsed = parse_baseline(&baseline_json(&entry)).expect("valid");
        let rendered = render_baseline(&parsed).expect("render");
        assert!(rendered.contains("\"target\": \"//x\""), "{rendered}");
    }
}
