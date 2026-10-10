use std::collections::{BTreeMap, BTreeSet};

pub const BASELINE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaselineError {
    #[error("baseline is not JSON: {detail}")]
    Json { detail: String },
    #[error("unsupported baseline version {found}: want {BASELINE_VERSION}")]
    UnsupportedVersion { found: u32 },
    #[error("baseline entry {index} is malformed: {reason}")]
    MalformedEntry { index: usize, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineDoc {
    version: u32,
    #[serde(default)]
    entries: Vec<EntryDoc>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryDoc {
    tool: String,
    #[serde(default)]
    rule: String,
    path: String,
    message: String,
    context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Fingerprint {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub message: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct BaselineFile {
    version: u32,
    entries: Vec<Fingerprint>,
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn context_digest(context: &str) -> String {
    format!("{:016x}", fnv1a64(context.as_bytes()))
}

pub fn normalize_message(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_hex_digest(text: &str) -> bool {
    text.len() == 16 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn context_line(source: &str, offset: u64) -> Option<&str> {
    let offset = usize::try_from(offset).ok()?;
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let line = source.get(start..end)?;
    Some(line.strip_suffix('\r').unwrap_or(line))
}

pub fn fingerprint(
    tool: &str,
    rule: &str,
    path: &str,
    message: &str,
    context: Option<&str>,
) -> Fingerprint {
    Fingerprint {
        tool: tool.to_owned(),
        rule: rule.to_owned(),
        path: path.to_owned(),
        message: normalize_message(message),
        context: context_digest(context.unwrap_or("")),
    }
}

fn check_entry(entry: &EntryDoc, index: usize) -> Result<Fingerprint, BaselineError> {
    let malformed = |reason: &str| BaselineError::MalformedEntry {
        index,
        reason: reason.to_owned(),
    };
    if entry.tool.is_empty() {
        return Err(malformed("empty tool: want a non-empty tool identity"));
    }
    if entry.path.is_empty() {
        return Err(malformed("empty path: want a workspace-relative path"));
    }
    if normalize_message(&entry.message).is_empty() {
        return Err(malformed("empty message: want a non-empty message"));
    }
    if !is_hex_digest(&entry.context) {
        return Err(malformed(
            "bad context: want the 16-character hex digest of the finding line",
        ));
    }
    Ok(Fingerprint {
        tool: entry.tool.clone(),
        rule: entry.rule.clone(),
        path: entry.path.clone(),
        message: normalize_message(&entry.message),
        context: entry.context.clone(),
    })
}

pub fn parse_baseline(bytes: &[u8]) -> Result<Vec<Fingerprint>, BaselineError> {
    let doc: BaselineDoc =
        serde_json::from_slice(bytes).map_err(|error| BaselineError::Json {
            detail: error.to_string(),
        })?;
    if doc.version != BASELINE_VERSION {
        return Err(BaselineError::UnsupportedVersion {
            found: doc.version,
        });
    }
    doc.entries
        .iter()
        .enumerate()
        .map(|(index, entry)| check_entry(entry, index))
        .collect()
}

pub struct BaselineMatch {
    pub suppressed: Vec<bool>,
    pub remaining: BTreeMap<Fingerprint, u64>,
}

pub fn match_baseline(
    current: &[Fingerprint],
    entries: &[Fingerprint],
) -> BaselineMatch {
    let mut remaining: BTreeMap<Fingerprint, u64> = BTreeMap::new();
    for entry in entries {
        *remaining.entry(entry.clone()).or_default() += 1;
    }
    let mut suppressed = Vec::with_capacity(current.len());
    for finding in current {
        let count = remaining.get(finding).copied().unwrap_or(0);
        if count > 0 {
            remaining.insert(finding.clone(), count - 1);
            suppressed.push(true);
        } else {
            suppressed.push(false);
        }
    }
    BaselineMatch {
        suppressed,
        remaining,
    }
}

pub struct BaselineScope {
    pub complete: bool,
    pub analyzed: BTreeSet<(String, String)>,
    pub existing: BTreeSet<String>,
}

pub fn stale_entries(
    entries: &[Fingerprint],
    remaining: &BTreeMap<Fingerprint, u64>,
    scope: &BaselineScope,
) -> Vec<usize> {
    if !scope.complete {
        return Vec::new();
    }
    let mut by_fingerprint: BTreeMap<&Fingerprint, Vec<usize>> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        by_fingerprint.entry(entry).or_default().push(index);
    }
    let mut stale = Vec::new();
    for (finding, indices) in &by_fingerprint {
        let unmatched = remaining.get(*finding).copied().unwrap_or(0) as usize;
        if unmatched == 0 {
            continue;
        }
        let proven = scope
            .analyzed
            .contains(&(finding.tool.clone(), finding.path.clone()))
            || !scope.existing.contains(&finding.path);
        if !proven {
            continue;
        }
        let keep = indices.len().saturating_sub(unmatched);
        stale.extend(indices.iter().skip(keep).copied());
    }
    stale.sort();
    stale
}

pub fn render_baseline(entries: &[Fingerprint]) -> Result<String, BaselineError> {
    let mut ordered = entries.to_vec();
    ordered.sort();
    let file = BaselineFile {
        version: BASELINE_VERSION,
        entries: ordered,
    };
    let pretty = serde_json::to_string_pretty(&file).map_err(|error| BaselineError::Json {
        detail: error.to_string(),
    })?;
    Ok(crate::ensure_ascii(&pretty) + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(tool: &str, message: &str) -> Fingerprint {
        fingerprint(tool, "tool/rule", "src/a.py", message, Some("x = 1"))
    }

    #[test]
    fn message_normalization_collapses_whitespace() {
        assert_eq!(normalize_message("  unused  import\n next"), "unused import next");
        assert_eq!(normalize_message(""), "");
        assert_eq!(fingerprint("t", "r", "p", "  a  b ", Some("c")).message, "a b");
    }

    #[test]
    fn context_digest_is_stable_hex() {
        assert_eq!(context_digest("x = 1"), context_digest("x = 1"));
        assert_ne!(context_digest("x = 1"), context_digest("x = 2"));
        let digest = context_digest("x = 1");
        assert_eq!(digest.len(), 16);
        assert!(is_hex_digest(&digest));
    }

    #[test]
    fn context_line_selects_the_covering_line() {
        let source = "first\nsecond line\nthird";
        assert_eq!(context_line(source, 0), Some("first"));
        assert_eq!(context_line(source, 5), Some("first"));
        assert_eq!(context_line(source, 6), Some("second line"));
        assert_eq!(context_line(source, 17), Some("third"));
        assert_eq!(context_line(source, 22), Some("third"));
        assert_eq!(context_line(source, 23), None);
        assert_eq!(context_line("a\r\nb", 0), Some("a"));
        assert_eq!(context_line("h\u{e9}llo", 1), None);
        assert_eq!(context_line("", 0), Some(""));
    }

    #[test]
    fn moved_lines_match_while_offsets_do_not_matter() {
        let before = fingerprint("ruff", "F401", "src/a.py", "unused", Some("import os"));
        let after = fingerprint("ruff", "F401", "src/a.py", "unused", Some("import os"));
        assert_eq!(before, after);
        let renamed = fingerprint("ruff", "F401", "src/b.py", "unused", Some("import os"));
        assert_ne!(before, renamed);
        let retooled = fingerprint("ty", "F401", "src/a.py", "unused", Some("import os"));
        assert_ne!(before, retooled);
        let reruled = fingerprint("ruff", "F841", "src/a.py", "unused", Some("import os"));
        assert_ne!(before, reruled);
        let reworded = fingerprint("ruff", "F401", "src/a.py", "used", Some("import os"));
        assert_ne!(before, reworded);
        let moved_context = fingerprint("ruff", "F401", "src/a.py", "unused", Some("import sys"));
        assert_ne!(before, moved_context);
    }

    #[test]
    fn multiset_matching_limits_suppression_to_recorded_counts() {
        let current = vec![finding("ruff", "a"), finding("ruff", "a"), finding("ruff", "b")];
        let entries = vec![finding("ruff", "a")];
        let matched = match_baseline(&current, &entries);
        assert_eq!(matched.suppressed, vec![true, false, false]);
        assert_eq!(matched.remaining.len(), 0);
        let matched = match_baseline(&current, &current);
        assert_eq!(matched.suppressed, vec![true, true, true]);
        assert!(matched.remaining.values().all(|count| *count == 0));
    }

    #[test]
    fn roundtrip_preserves_sorted_entries() {
        let entries = vec![finding("b", "m"), finding("a", "m")];
        let rendered = render_baseline(&entries).expect("render");
        assert!(rendered.ends_with('\n'));
        let parsed = parse_baseline(rendered.as_bytes()).expect("parse");
        assert_eq!(parsed, vec![finding("a", "m"), finding("b", "m")]);
        let empty = render_baseline(&[]).expect("empty renders");
        assert_eq!(parse_baseline(empty.as_bytes()).expect("empty parses"), vec![]);
    }

    #[test]
    fn refresh_output_stays_ascii() {
        let entries = vec![fingerprint("t", "r", "caf\u{e9}.py", "m", Some("x"))];
        let rendered = render_baseline(&entries).expect("render");
        assert!(rendered.contains("caf\\u00e9"));
        assert!(!rendered.contains("caf\u{e9}"));
        assert_eq!(parse_baseline(rendered.as_bytes()).expect("parse"), entries);
    }

    #[test]
    fn malformed_entries_fail_with_the_index_named() {
        let version = r#"{"version": 1, "entries": ["#;
        let close = "]}";
        for (body, reason) in [
            (r#"{"tool": "", "path": "a.py", "message": "m", "context": "0123456789abcdef"}"#, "empty tool"),
            (r#"{"tool": "t", "path": "", "message": "m", "context": "0123456789abcdef"}"#, "empty path"),
            (r#"{"tool": "t", "path": "a.py", "message": "  ", "context": "0123456789abcdef"}"#, "empty message"),
            (r#"{"tool": "t", "path": "a.py", "message": "m", "context": "short"}"#, "bad context"),
            (r#"{"tool": "t", "path": "a.py", "message": "m"}"#, "bad context"),
        ] {
            let error = parse_baseline(format!("{version}{body}{close}").as_bytes())
                .expect_err(reason);
            assert_eq!(error, BaselineError::MalformedEntry { index: 0, reason: reason.to_owned() }, "{reason}");
        }
        let error = parse_baseline(br#"{"version": 2, "entries": []}"#).expect_err("version");
        assert_eq!(error, BaselineError::UnsupportedVersion { found: 2 });
        assert!(matches!(
            parse_baseline(b"{not json"),
            Err(BaselineError::Json { .. })
        ));
        assert!(matches!(
            parse_baseline(br#"{"version": 1, "entries": [], "extra": true}"#),
            Err(BaselineError::Json { .. })
        ));
    }

    #[test]
    fn stale_detection_needs_complete_in_scope_analysis() {
        let entries = vec![finding("ruff", "gone"), finding("other", "kept")];
        let matched = match_baseline(&[], &entries);
        let analyzed: BTreeSet<(String, String)> = [("ruff".to_owned(), "src/a.py".to_owned())]
            .into_iter()
            .collect();
        let existing: BTreeSet<String> = ["src/a.py".to_owned()].into_iter().collect();
        let scope = BaselineScope { complete: true, analyzed, existing };
        assert_eq!(stale_entries(&entries, &matched.remaining, &scope), vec![0]);
        let incomplete = BaselineScope {
            complete: false,
            analyzed: BTreeSet::new(),
            existing: BTreeSet::new(),
        };
        assert!(stale_entries(&entries, &matched.remaining, &incomplete).is_empty());
    }

    #[test]
    fn deleted_files_and_duplicates_resolve_deterministically() {
        let entries = vec![finding("ruff", "dup"), finding("ruff", "dup")];
        let matched = match_baseline(&[finding("ruff", "dup")], &entries);
        assert_eq!(matched.suppressed, vec![true]);
        let scope = BaselineScope {
            complete: true,
            analyzed: BTreeSet::new(),
            existing: ["src/a.py".to_owned()].into_iter().collect(),
        };
        assert_eq!(stale_entries(&entries, &matched.remaining, &scope), vec![1]);
        let scope = BaselineScope {
            complete: true,
            analyzed: BTreeSet::new(),
            existing: BTreeSet::new(),
        };
        let matched = match_baseline(&[], &entries);
        assert_eq!(stale_entries(&entries, &matched.remaining, &scope), vec![0, 1]);
    }
}
