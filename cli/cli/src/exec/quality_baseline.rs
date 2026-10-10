use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use dx_output::DiagnosticEvent;

pub const BASELINE_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaselineError {
    #[error("baseline {path:?} is missing: run with --apply to create it")]
    Missing { path: String },
    #[error("baseline {path:?} is unreadable: {detail}")]
    Unreadable { path: String, detail: String },
    #[error("baseline {path:?} is malformed: {detail}")]
    Malformed { path: String, detail: String },
    #[error("baseline selection {path:?} escapes the workspace")]
    EscapesWorkspace { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineEntryRaw {
    #[serde(default)]
    tool: String,
    #[serde(default)]
    rule: Option<String>,
    #[serde(default)]
    path: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    context_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineFileRaw {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    entries: Vec<BaselineEntryRaw>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub tool: String,
    pub rule: Option<String>,
    pub path: String,
    pub message: String,
    pub context_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineCounts {
    pub total: usize,
    pub suppressed: usize,
    pub new: usize,
    pub stale: usize,
}

pub fn normalize_message(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_path(path: &str) -> Option<String> {
    let mut owned = path.trim().replace('\\', "/");
    while let Some(rest) = owned.strip_prefix("./") {
        owned = rest.to_owned();
    }
    if owned.is_empty() {
        return None;
    }
    if dx_path::reject_reason(&owned).is_some() {
        return None;
    }
    Some(owned)
}

fn context_lines(source: &str, range: Option<(u64, u64)>) -> Option<String> {
    let (start, end) = range?;
    let len = source.len() as u64;
    if start > end || end > len {
        return None;
    }
    if !source.is_char_boundary(start as usize) || !source.is_char_boundary(end as usize) {
        return None;
    }
    let mut lines: Vec<&str> = Vec::new();
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let line_start = offset as u64;
        let line_end = (offset + line.len()) as u64;
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        if line_end > start && line_start < end.max(start + 1).min(line_end + 1) {
            lines.push(body);
        } else if start >= line_start && start < line_end {
            lines.push(body);
        }
        offset += line.len();
    }
    if lines.is_empty() {
        let last = source.rsplit('\n').next().unwrap_or("");
        lines.push(last.strip_suffix('\r').unwrap_or(last));
    }
    Some(lines.join("\n"))
}

pub fn context_digest_for(source_bytes: &[u8], range: Option<(u64, u64)>) -> Option<String> {
    let source = std::str::from_utf8(source_bytes).ok()?;
    match range {
        Some(_) => {
            let context = context_lines(source, range)?;
            Some(dx_digest::to_hex(&dx_digest::blake3(context.as_bytes())))
        }
        None => Some(dx_digest::to_hex(&dx_digest::blake3(source_bytes))),
    }
}

pub fn fingerprint_for(
    tool: &str,
    rule: Option<&str>,
    path: Option<&str>,
    message: &str,
    source_bytes: Option<&[u8]>,
    range: Option<(u64, u64)>,
) -> Option<Fingerprint> {
    if tool.trim().is_empty() {
        return None;
    }
    let path = normalize_path(path?)?;
    let message = normalize_message(message);
    if message.is_empty() {
        return None;
    }
    let rule = rule
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(ToString::to_string);
    let bytes = source_bytes?;
    let context_digest = context_digest_for(bytes, range)?;
    dx_digest::parse_hex(&context_digest).ok()?;
    Some(Fingerprint {
        tool: tool.trim().to_owned(),
        rule,
        path,
        message,
        context_digest,
    })
}

pub fn fingerprint_diagnostic(
    diagnostic: &DiagnosticEvent,
    sources: &BTreeMap<String, Vec<u8>>,
) -> Option<Fingerprint> {
    let path = diagnostic.path.as_deref()?;
    let bytes = sources.get(path)?;
    fingerprint_for(
        &diagnostic.tool,
        diagnostic.rule.as_deref(),
        Some(path),
        &diagnostic.message,
        Some(bytes),
        diagnostic.range,
    )
}

fn validate_entry(path: &str, entry: &BaselineEntryRaw) -> Result<Fingerprint, String> {
    if entry.tool.trim().is_empty() {
        return Err("entry has an empty tool".to_owned());
    }
    let normalized_path = normalize_path(&entry.path)
        .ok_or_else(|| format!("entry has an invalid path {:?}", entry.path))?;
    let message = normalize_message(&entry.message);
    if message.is_empty() {
        return Err("entry has an empty message".to_owned());
    }
    let rule = entry
        .rule
        .as_deref()
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(ToString::to_string);
    if dx_digest::parse_hex(entry.context_digest.trim()).is_err() {
        return Err(format!(
            "entry for {:?} has an invalid context digest",
            entry.path
        ));
    }
    let _ = path;
    Ok(Fingerprint {
        tool: entry.tool.trim().to_owned(),
        rule,
        path: normalized_path,
        message,
        context_digest: entry.context_digest.trim().to_owned(),
    })
}

pub fn parse_baseline_bytes(path: &str, bytes: &[u8]) -> Result<Vec<Fingerprint>, BaselineError> {
    let text = std::str::from_utf8(bytes).map_err(|_| BaselineError::Malformed {
        path: path.to_owned(),
        detail: "file is not UTF-8".to_owned(),
    })?;
    let raw: BaselineFileRaw = serde_json::from_str(text).map_err(|error| {
        BaselineError::Malformed {
            path: path.to_owned(),
            detail: error.to_string(),
        }
    })?;
    if raw.schema != BASELINE_SCHEMA {
        return Err(BaselineError::Malformed {
            path: path.to_owned(),
            detail: format!("unsupported schema {}, want {BASELINE_SCHEMA}", raw.schema),
        });
    }
    let mut out = Vec::with_capacity(raw.entries.len());
    for entry in &raw.entries {
        match validate_entry(path, entry) {
            Ok(fingerprint) => out.push(fingerprint),
            Err(detail) => {
                return Err(BaselineError::Malformed {
                    path: path.to_owned(),
                    detail,
                })
            }
        }
    }
    Ok(out)
}

pub fn resolve_baseline_path(workspace: &Path, selection: &str) -> Result<String, BaselineError> {
    let trimmed = selection.trim().replace('\\', "/");
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || dx_path::is_absolute(&trimmed)
        || dx_path::reject_reason(&trimmed).is_some()
    {
        return Err(BaselineError::EscapesWorkspace {
            path: selection.to_owned(),
        });
    }
    let _ = workspace;
    Ok(trimmed)
}

pub fn load_baseline(
    workspace: &Path,
    selection: &str,
) -> Result<Vec<Fingerprint>, BaselineError> {
    let rel = resolve_baseline_path(workspace, selection)?;
    let full = workspace.join(&rel);
    let bytes = std::fs::read(&full).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            BaselineError::Missing {
                path: rel.clone(),
            }
        } else {
            BaselineError::Unreadable {
                path: rel.clone(),
                detail: error.to_string(),
            }
        }
    })?;
    parse_baseline_bytes(&rel, &bytes)
}

pub struct MatchOutcome {
    pub suppressed: Vec<usize>,
    pub stale: Vec<usize>,
}

pub fn match_fingerprints(
    current: &[Fingerprint],
    baseline: &[Fingerprint],
) -> MatchOutcome {
    let mut by_key: BTreeMap<String, VecDeque<usize>> = BTreeMap::new();
    for (index, entry) in baseline.iter().enumerate() {
        by_key.entry(key(entry)).or_default().push_back(index);
    }
    let mut suppressed = Vec::new();
    let mut used = BTreeSet::new();
    for (index, finding) in current.iter().enumerate() {
        let key = key(finding);
        let hit = by_key
            .get_mut(&key)
            .and_then(|queue| queue.pop_front());
        if let Some(base) = hit {
            used.insert(base);
            suppressed.push(index);
        }
    }
    let mut stale = Vec::new();
    for (index, _) in baseline.iter().enumerate() {
        if !used.contains(&index) {
            stale.push(index);
        }
    }
    MatchOutcome { suppressed, stale }
}

fn key(fingerprint: &Fingerprint) -> String {
    let rule = fingerprint.rule.as_deref().unwrap_or("");
    format!(
        "{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
        fingerprint.tool, rule, fingerprint.path, fingerprint.message, fingerprint.context_digest
    )
}

pub fn in_proven_scope(
    entry_path: &str,
    complete: bool,
    terminal_digests: &BTreeSet<String>,
    workspace: &Path,
) -> bool {
    if !complete {
        return false;
    }
    if terminal_digests.contains(entry_path) {
        return true;
    }
    if !workspace.join(entry_path).exists() {
        return true;
    }
    false
}

pub fn proven_stale(
    baseline: &[Fingerprint],
    stale: &[usize],
    complete: bool,
    terminal_digests: &BTreeSet<String>,
    workspace: &Path,
) -> Vec<usize> {
    if !complete {
        return Vec::new();
    }
    stale
        .iter()
        .copied()
        .filter(|index| {
            baseline
                .get(*index)
                .is_some_and(|entry| in_proven_scope(&entry.path, complete, terminal_digests, workspace))
        })
        .collect()
}

pub fn render_baseline_file(entries: &[Fingerprint]) -> Result<String, BaselineError> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.tool.cmp(&b.tool))
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.context_digest.cmp(&b.context_digest))
    });
    let values: Vec<serde_json::Value> = sorted
        .iter()
        .map(|entry| {
            let mut map = serde_json::Map::new();
            map.insert(
                "context_digest".to_owned(),
                serde_json::Value::String(entry.context_digest.clone()),
            );
            map.insert(
                "message".to_owned(),
                serde_json::Value::String(entry.message.clone()),
            );
            map.insert("path".to_owned(), serde_json::Value::String(entry.path.clone()));
            if let Some(rule) = &entry.rule {
                map.insert("rule".to_owned(), serde_json::Value::String(rule.clone()));
            }
            map.insert("tool".to_owned(), serde_json::Value::String(entry.tool.clone()));
            serde_json::Value::Object(map)
        })
        .collect();
    let document = serde_json::json!({"entries": values, "schema": BASELINE_SCHEMA});
    serde_json::to_string_pretty(&document).map(|text| text + "\n").map_err(|_| {
        BaselineError::Malformed {
            path: String::new(),
            detail: "cannot render baseline".to_owned(),
        }
    })
}

pub struct BaselineApplied {
    pub suppressed: Vec<bool>,
    pub counts: Option<BaselineCounts>,
    pub stale_paths: Vec<String>,
    pub error: Option<String>,
    pub selection: Option<String>,
}

fn read_sources(
    workspace: &Path,
    status: &[DiagnosticEvent],
) -> BTreeMap<String, Vec<u8>> {
    let mut sources = BTreeMap::new();
    for diagnostic in status {
        let Some(path) = diagnostic.path.as_deref() else {
            continue;
        };
        if sources.contains_key(path) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(workspace.join(path)) {
            sources.insert(path.to_owned(), bytes);
        }
    }
    sources
}

pub fn apply_baseline(
    workspace: &Path,
    status: &[DiagnosticEvent],
    complete: bool,
    terminal_digests: &BTreeMap<String, [u8; 32]>,
    apply: bool,
) -> BaselineApplied {
    let selection = match dx_adopt::defaults::load_defaults(workspace) {
        Ok((defaults, _)) => defaults.baseline,
        Err(error) => {
            return BaselineApplied {
                suppressed: vec![false; status.len()],
                counts: None,
                stale_paths: Vec::new(),
                error: Some(format!("invalid dx.toml: {error}")),
                selection: None,
            }
        }
    };
    let Some(selection) = selection.filter(|text| !text.trim().is_empty()) else {
        return BaselineApplied {
            suppressed: vec![false; status.len()],
            counts: None,
            stale_paths: Vec::new(),
            error: None,
            selection: None,
        };
    };
    let rel = match resolve_baseline_path(workspace, &selection) {
        Ok(rel) => rel,
        Err(error) => {
            return BaselineApplied {
                suppressed: vec![false; status.len()],
                counts: None,
                stale_paths: Vec::new(),
                error: Some(error.to_string()),
                selection: Some(selection),
            }
        }
    };
    let baseline = match load_baseline(workspace, &selection) {
        Ok(entries) => entries,
        Err(BaselineError::Missing { path }) => {
            if !apply || !complete {
                return BaselineApplied {
                    suppressed: vec![false; status.len()],
                    counts: Some(BaselineCounts {
                        total: status.len(),
                        suppressed: 0,
                        new: status.len(),
                        stale: 0,
                    }),
                    stale_paths: Vec::new(),
                    error: Some(BaselineError::Missing { path }.to_string()),
                    selection: Some(selection),
                };
            }
            Vec::new()
        }
        Err(error) => {
            return BaselineApplied {
                suppressed: vec![false; status.len()],
                counts: Some(BaselineCounts {
                    total: status.len(),
                    suppressed: 0,
                    new: status.len(),
                    stale: 0,
                }),
                stale_paths: Vec::new(),
                error: Some(error.to_string()),
                selection: Some(selection),
            }
        }
    };
    let sources = read_sources(workspace, status);
    let mut current: Vec<Option<Fingerprint>> = Vec::with_capacity(status.len());
    for diagnostic in status {
        if diagnostic.path.is_none() {
            current.push(None);
            continue;
        }
        current.push(fingerprint_diagnostic(diagnostic, &sources));
    }
    let present: Vec<Fingerprint> = current.iter().flatten().cloned().collect();
    let present_index: Vec<usize> = current
        .iter()
        .enumerate()
        .filter_map(|(index, fp)| fp.as_ref().map(|_| index))
        .collect();
    let outcome = match_fingerprints(&present, &baseline);
    let mut suppressed = vec![false; status.len()];
    for position in outcome.suppressed {
        if let Some(status_index) = present_index.get(position) {
            suppressed[*status_index] = true;
        }
    }
    let covered: BTreeSet<String> = terminal_digests.keys().cloned().collect();
    let proven = proven_stale(&baseline, &outcome.stale, complete, &covered, workspace);
    let stale_paths: Vec<String> = proven
        .iter()
        .filter_map(|index| baseline.get(*index).map(|entry| entry.path.clone()))
        .collect();
    if !complete {
        let counts = BaselineCounts {
            total: status.len(),
            suppressed: 0,
            new: status.len(),
            stale: 0,
        };
        return BaselineApplied {
            suppressed: vec![false; status.len()],
            counts: Some(counts),
            stale_paths: Vec::new(),
            error: None,
            selection: Some(selection),
        };
    }
    if apply {
        let mut next: Vec<Fingerprint> = present.clone();
        let proven_set: BTreeSet<usize> = proven.into_iter().collect();
        for (index, entry) in baseline.iter().enumerate() {
            if outcome.stale.contains(&index) && !proven_set.contains(&index) {
                next.push(entry.clone());
            }
        }
        let mut seen = BTreeSet::new();
        next.retain(|entry| seen.insert(key(entry)));
        match render_baseline_file(&next) {
            Ok(text) => {
                let full = workspace.join(&rel);
                let parent_ok = full
                    .parent()
                    .is_some_and(|parent| parent.is_dir());
                let write_result = if parent_ok {
                    std::fs::write(&full, text.as_bytes())
                } else {
                    Err(std::io::Error::other("parent directory does not exist"))
                };
                match write_result {
                    Ok(()) => {
                        let refreshed_flags: Vec<bool> =
                            current.iter().map(|fp| fp.is_some()).collect();
                        let suppressed_count =
                            refreshed_flags.iter().filter(|flag| **flag).count();
                        return BaselineApplied {
                            suppressed: refreshed_flags,
                            counts: Some(BaselineCounts {
                                total: status.len(),
                                suppressed: suppressed_count,
                                new: status.len() - suppressed_count,
                                stale: 0,
                            }),
                            stale_paths: Vec::new(),
                            error: None,
                            selection: Some(selection),
                        };
                    }
                    Err(error) => {
                        let held = suppressed.iter().filter(|flag| **flag).count();
                        return BaselineApplied {
                            suppressed,
                            counts: Some(BaselineCounts {
                                total: status.len(),
                                suppressed: held,
                                new: status.len() - held,
                                stale: stale_paths.len(),
                            }),
                            stale_paths,
                            error: Some(format!(
                                "baseline {rel:?} cannot be written: {error}"
                            )),
                            selection: Some(selection),
                        };
                    }
                }
            }
            Err(error) => {
                return BaselineApplied {
                    suppressed,
                    counts: None,
                    stale_paths,
                    error: Some(error.to_string()),
                    selection: Some(selection),
                };
            }
        }
    }
    let suppressed_count = suppressed.iter().filter(|flag| **flag).count();
    BaselineApplied {
        suppressed,
        counts: Some(BaselineCounts {
            total: status.len(),
            suppressed: suppressed_count,
            new: status.len() - suppressed_count,
            stale: stale_paths.len(),
        }),
        stale_paths,
        error: None,
        selection: Some(selection),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(
        tool: &str,
        rule: Option<&str>,
        path: &str,
        message: &str,
        context: &str,
    ) -> Fingerprint {
        Fingerprint {
            tool: tool.to_owned(),
            rule: rule.map(ToString::to_string),
            path: path.to_owned(),
            message: message.to_owned(),
            context_digest: context.to_owned(),
        }
    }

    fn digest_of(text: &str) -> String {
        dx_digest::to_hex(&dx_digest::blake3(text.as_bytes()))
    }

    #[test]
    fn messages_normalize_whitespace() {
        assert_eq!(normalize_message("  a\tb\n c  "), "a b c");
        assert_eq!(normalize_message("single"), "single");
        assert_eq!(normalize_message("   "), "");
    }

    #[test]
    fn paths_normalize_and_reject() {
        assert_eq!(normalize_path("src/a.py"), Some("src/a.py".to_owned()));
        assert_eq!(normalize_path("./src/a.py"), Some("src/a.py".to_owned()));
        assert_eq!(normalize_path("src\\a.py"), Some("src/a.py".to_owned()));
        assert_eq!(normalize_path(""), None);
        assert_eq!(normalize_path("/abs"), None);
        assert_eq!(normalize_path("../escape"), None);
        assert_eq!(normalize_path("a/../b"), None);
    }

    #[test]
    fn ranged_context_ignores_offsets() {
        let source = b"first line\nsecond line\nthird line\n";
        let first = context_digest_for(source, Some((0, 5))).expect("first");
        let moved = context_digest_for(source, Some((23, 28))).expect("moved");
        assert_ne!(first, moved);
        let same_line = context_digest_for(source, Some((11, 17))).expect("second");
        let same_shifted = context_digest_for(source, Some((11, 12))).expect("prefix");
        assert_eq!(same_line, same_shifted);
        assert_eq!(first, digest_of("first line"));
        assert_eq!(same_line, digest_of("second line"));
    }

    #[test]
    fn multiline_range_joins_lines() {
        let source = b"a\nb\nc\n";
        let digest = context_digest_for(source, Some((0, 4))).expect("two lines");
        assert_eq!(digest, digest_of("a\nb"));
    }

    #[test]
    fn unranged_context_covers_the_file() {
        let source = b"x = 1\n";
        assert_eq!(
            context_digest_for(source, None).expect("file"),
            digest_of("x = 1\n")
        );
        assert!(context_digest_for(b"\xff", Some((0, 1))).is_none());
        assert!(context_digest_for(b"\xff", None).is_none());
    }

    #[test]
    fn fingerprints_reject_paths_and_messages() {
        let source = b"x = 1\n";
        assert!(fingerprint_for("", None, Some("src/a.py"), "m", Some(source), None).is_none());
        assert!(fingerprint_for("t", None, None, "m", Some(source), None).is_none());
        assert!(fingerprint_for("t", None, Some(""), "m", Some(source), None).is_none());
        assert!(fingerprint_for("t", None, Some("src/a.py"), "  ", Some(source), None).is_none());
        assert!(fingerprint_for("t", None, Some("src/a.py"), "m", None, None).is_none());
        let made =
            fingerprint_for(" t ", Some(" r "), Some("./src/a.py"), " a  b ", Some(source), None)
                .expect("normalized");
        assert_eq!(made.tool, "t");
        assert_eq!(made.rule, Some("r".to_owned()));
        assert_eq!(made.path, "src/a.py");
        assert_eq!(made.message, "a b");
    }

    #[test]
    fn multiset_matching_limits_duplicates() {
        let digest = digest_of("line");
        let one = finding("t", Some("r"), "src/a.py", "m", &digest);
        let current = vec![one.clone(), one.clone(), one.clone()];
        let baseline = vec![one.clone(), one.clone()];
        let outcome = match_fingerprints(&current, &baseline);
        assert_eq!(outcome.suppressed.len(), 2);
        assert_eq!(outcome.stale.len(), 0);
    }

    #[test]
    fn tool_rule_path_changes_break_identity() {
        let digest = digest_of("line");
        let base = finding("t", Some("r"), "src/a.py", "m", &digest);
        for changed in [
            finding("other", Some("r"), "src/a.py", "m", &digest),
            finding("t", Some("other"), "src/a.py", "m", &digest),
            finding("t", Some("r"), "src/b.py", "m", &digest),
            finding("t", Some("r"), "src/a.py", "other", &digest),
            finding("t", Some("r"), "src/a.py", "m", &digest_of("changed")),
        ] {
            let outcome = match_fingerprints(std::slice::from_ref(&changed), std::slice::from_ref(&base));
            assert_eq!(outcome.suppressed.len(), 0, "{changed:?}");
            assert_eq!(outcome.stale.len(), 1);
        }
    }

    #[test]
    fn baseline_file_round_trips_sorted() {
        let entries = vec![
            finding("b", None, "src/b.py", "m", &digest_of("x")),
            finding("a", Some("r"), "src/a.py", "m", &digest_of("y")),
        ];
        let text = render_baseline_file(&entries).expect("render");
        assert!(text.ends_with('\n'));
        let parsed = parse_baseline_bytes("base.json", text.as_bytes()).expect("parse");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].path, "src/a.py");
        assert_eq!(parsed[1].path, "src/b.py");
    }

    #[test]
    fn malformed_files_fail() {
        assert!(parse_baseline_bytes("b", b"not json").is_err());
        assert!(parse_baseline_bytes("b", b"{\"schema\":1,\"entries\":[{}]}").is_err());
        assert!(parse_baseline_bytes("b", b"{\"schema\":2,\"entries\":[]}").is_err());
        assert!(parse_baseline_bytes("b", b"{\"schema\":1,\"entries\":[],\"extra\":1}").is_err());
        assert!(parse_baseline_bytes(
            "b",
            b"{\"schema\":1,\"entries\":[{\"tool\":\"t\",\"path\":\"src/a.py\",\"message\":\"m\",\"context_digest\":\"zz\",\"extra\":1}]}"
        )
        .is_err());
        assert!(parse_baseline_bytes(
            "b",
            b"{\"schema\":1,\"entries\":[{\"tool\":\"\",\"path\":\"src/a.py\",\"message\":\"m\",\"context_digest\":\"00\"}]}"
        )
        .is_err());
    }

    #[test]
    fn selections_stay_inside_the_workspace() {
        let root = std::path::Path::new("/repo");
        assert_eq!(
            resolve_baseline_path(root, "quality/base.json").expect("relative"),
            "quality/base.json"
        );
        assert!(resolve_baseline_path(root, "/abs.json").is_err());
        assert!(resolve_baseline_path(root, "../out.json").is_err());
        assert!(resolve_baseline_path(root, "").is_err());
    }

    #[test]
    fn stale_scope_needs_complete_covered_files() {
        let workspace = dx_test_scratch::scratch("dx-baseline-scope-");
        let root = workspace.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).expect("dirs");
        std::fs::write(root.join("src/a.py"), "x = 1\n").expect("source");
        std::fs::write(root.join("src/c.py"), "y = 2\n").expect("outside scope");
        let mut covered = BTreeSet::new();
        covered.insert("src/a.py".to_owned());
        assert!(in_proven_scope("src/a.py", true, &covered, &root));
        assert!(!in_proven_scope("src/a.py", false, &covered, &root));
        assert!(!in_proven_scope("src/c.py", true, &covered, &root));
        assert!(in_proven_scope("src/deleted.py", true, &covered, &root));
        let outside = vec![finding("t", None, "src/c.py", "m", &digest_of("x"))];
        assert!(proven_stale(&outside, &[0], true, &covered, &root).is_empty());
        assert!(proven_stale(&outside, &[0], false, &covered, &root).is_empty());
        let deleted = vec![finding("t", None, "src/deleted.py", "m", &digest_of("x"))];
        assert_eq!(proven_stale(&deleted, &[0], true, &covered, &root), vec![0]);
        workspace.close().expect("cleanup");
    }

    fn baseline_harness(
        name: &str,
        source: &str,
        diagnostics: Vec<quality_result::proto::Diagnostic>,
    ) -> super::super::test_support::Harness {
        let mut harness = super::super::test_support::Harness::new(name);
        harness.write_source("src/a.py", source);
        let bytes = harness.result_full(
            diagnostics,
            Vec::new(),
            Vec::new(),
            vec![quality_result::proto::FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: dx_digest::blake3(source.as_bytes()).to_vec(),
            }],
        );
        harness.results.insert("//test:corpus".to_owned(), bytes);
        harness
    }

    fn write_baseline(
        harness: &super::super::test_support::Harness,
        entries: &[Fingerprint],
    ) {
        let text = render_baseline_file(entries).expect("render baseline");
        harness.write_source("baseline.json", &text);
        harness.write_source("dx.toml", "[dx]\nbaseline = \"baseline.json\"\n");
    }

    fn fp_for(source: &[u8], range: Option<(u64, u64)>, message: &str) -> Fingerprint {
        fingerprint_for(
            "lint-tool",
            Some("lint-tool/rule"),
            Some("src/a.py"),
            message,
            Some(source),
            range,
        )
        .expect("fingerprint")
    }

    #[test]
    fn baselined_findings_stay_visible_but_pass() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-suppressed",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("(suppressed)"), "{out}");
        assert!(out.contains("Baseline: 1 total, 0 new, 1 suppressed."), "{out}");
    }

    #[test]
    fn new_findings_fail_with_counts() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-new",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        write_baseline(&harness, &[]);
        harness.write_source("dx.toml", "[dx]\nbaseline = \"baseline.json\"\n");
        std::fs::write(harness.workspace.join("baseline.json"), "{\"schema\":1,\"entries\":[]}\n")
            .expect("empty baseline");
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("Baseline: 1 total, 1 new, 0 suppressed."), "{out}");
        assert!(!out.contains("(suppressed)"), "{out}");
    }

    #[test]
    fn json_marks_suppressed_and_reports_counts() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-json",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, out, _) = harness.run(&["lint", "--check", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("ndjson");
        let diagnostic = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("diagnostic"))
            .expect("diagnostic");
        assert_eq!(diagnostic["suppressed"], serde_json::json!(true));
        let notice = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("notice")
                    && event["code"] == serde_json::json!("baseline")
            })
            .expect("baseline notice");
        assert!(notice["message"]
            .as_str()
            .expect("message")
            .contains("1 total"));
    }

    #[test]
    fn stale_entries_fail_in_covered_scope() {
        let source = "x = 1\n";
        let harness = baseline_harness("baseline-stale", source, Vec::new());
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("baseline_stale"), "{err}");
    }

    #[test]
    fn moved_lines_keep_their_suppression() {
        let before = "first line\nsecond line\nthird line\n";
        let after = "third line\nsecond line\nfirst line\n";
        let mut harness = super::super::test_support::Harness::new("baseline-moved");
        harness.write_source("src/a.py", after);
        let diagnostic = quality_result::proto::Diagnostic {
            severity: quality_result::proto::Severity::Warning as i32,
            message: "moved".to_owned(),
            tool_id: "lint-tool".to_owned(),
            rule_id: "lint-tool/rule".to_owned(),
            path: "src/a.py".to_owned(),
            start_byte: Some(after.find("second line").expect("line") as u64),
            end_byte: Some((after.find("second line").expect("line") + 6) as u64),
            fixable: false,
        };
        let bytes = harness.result_full(
            vec![diagnostic],
            Vec::new(),
            Vec::new(),
            vec![quality_result::proto::FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: dx_digest::blake3(after.as_bytes()).to_vec(),
            }],
        );
        harness.results.insert("//test:corpus".to_owned(), bytes);
        let entry = fp_for(before.as_bytes(), Some((11, 17)), "moved");
        write_baseline(&harness, &[entry]);
        let (code, out, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("(suppressed)"), "{out}");
    }

    #[test]
    fn duplicate_findings_need_matching_counts() {
        let source = "x = 1\n";
        let one = super::super::test_support::Harness::diagnostic("unused", false);
        let harness = baseline_harness("baseline-dup", source, vec![one.clone(), one]);
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("Baseline: 2 total, 1 new, 1 suppressed."), "{out}");
    }

    #[test]
    fn rename_and_rule_change_break_suppression() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-rename",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        let renamed = Fingerprint {
            tool: "lint-tool".to_owned(),
            rule: Some("lint-tool/rule".to_owned()),
            path: "src/b.py".to_owned(),
            message: "unused".to_owned(),
            context_digest: fp_for(source.as_bytes(), Some((0, 1)), "unused").context_digest,
        };
        write_baseline(&harness, &[renamed]);
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("baseline_stale"), "{err}");
    }

    #[test]
    fn malformed_baselines_fail() {
        for (name, body) in [
            ("json", "not json"),
            ("schema", "{\"schema\":2,\"entries\":[]}"),
            ("unknown", "{\"schema\":1,\"entries\":[],\"extra\":1}"),
            (
                "entry",
                "{\"schema\":1,\"entries\":[{\"tool\":\"\",\"path\":\"src/a.py\",\"message\":\"m\",\"context_digest\":\"00\"}]}",
            ),
        ] {
            let source = "x = 1\n";
            let harness = baseline_harness(
                &format!("baseline-bad-{name}"),
                source,
                vec![super::super::test_support::Harness::diagnostic("unused", false)],
            );
            harness.write_source("baseline.json", body);
            harness.write_source("dx.toml", "[dx]\nbaseline = \"baseline.json\"\n");
            let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
            assert_eq!(code, 1, "{name}: {err}");
            assert!(err.contains("baseline"), "{name}: {err}");
        }
    }

    #[test]
    fn missing_baseline_needs_apply_and_check_leaves_bytes() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-missing",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        harness.write_source("dx.toml", "[dx]\nbaseline = \"baseline.json\"\n");
        let (code, _, err) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(!harness.workspace.join("baseline.json").exists());
        let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        assert!(harness.workspace.join("baseline.json").exists());
        let before = std::fs::read(harness.workspace.join("baseline.json")).expect("bytes");
        let (code, _, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 0);
        let after = std::fs::read(harness.workspace.join("baseline.json")).expect("bytes");
        assert_eq!(before, after);
    }

    #[test]
    fn apply_refreshes_only_proven_scope() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-refresh",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        harness.write_source("src/c.py", "y = 2\n");
        let outside = Fingerprint {
            tool: "lint-tool".to_owned(),
            rule: Some("lint-tool/rule".to_owned()),
            path: "src/c.py".to_owned(),
            message: "old".to_owned(),
            context_digest: digest_of("y = 2\n"),
        };
        let stale = fp_for(source.as_bytes(), Some((0, 1)), "gone");
        write_baseline(&harness, &[outside.clone(), stale.clone()]);
        let (code, _, err) = harness.run(&["lint", "--apply", "--output=text"]);
        assert_eq!(code, 0, "{err}");
        let text = std::fs::read_to_string(harness.workspace.join("baseline.json")).expect("read");
        let entries = parse_baseline_bytes("baseline.json", text.as_bytes()).expect("parse");
        assert!(entries.contains(&outside));
        assert!(entries.contains(&fp_for(source.as_bytes(), Some((0, 1)), "unused")));
        assert!(!entries.contains(&stale));
    }

    #[test]
    fn incomplete_results_never_suppress() {
        let source = "x = 1\n";
        let mut harness = baseline_harness(
            "baseline-incomplete",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        harness.fail_target = true;
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, out, _) = harness.run(&["lint", "--check", "--output=text"]);
        assert_eq!(code, 1, "{out}");
        assert!(!out.contains("(suppressed)"), "{out}");
    }

    #[test]
    fn sarif_marks_baseline_state_and_counts() {
        let source = "x = 1\n";
        let harness = baseline_harness(
            "baseline-sarif",
            source,
            vec![super::super::test_support::Harness::diagnostic("unused", false)],
        );
        write_baseline(&harness, &[fp_for(source.as_bytes(), Some((0, 1)), "unused")]);
        let (code, _, err) = harness.run(&[
            "lint",
            "--check",
            "--output=text",
            "--report=sarif=out.sarif",
        ]);
        assert_eq!(code, 0, "{err}");
        let text = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
        assert!(text.contains("unchanged"), "{text}");
        assert!(text.contains("baselineSuppressed"), "{text}");
    }
}
