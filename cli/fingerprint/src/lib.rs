#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FingerprintError {
    #[error("fingerprint JSON serializes: {detail}")]
    Json { detail: String },
}

impl From<serde_json::Error> for FingerprintError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json {
            detail: error.to_string(),
        }
    }
}

pub fn to_json<T: serde::Serialize>(view: &T) -> Result<String, FingerprintError> {
    serde_json::to_string(view).map_err(FingerprintError::from)
}

pub fn ensure_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let code = c as u32;
        if code < 0x80 {
            out.push(c);
        } else if code <= 0xFFFF {
            out.push_str(&format!("\\u{code:04x}"));
        } else {
            let v = code - 0x10000;
            let high = 0xD800 + (v >> 10);
            let low = 0xDC00 + (v & 0x3FF);
            out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
        }
    }
    out
}

pub fn to_json_ascii_pretty(value: &serde_json::Value) -> Result<String, FingerprintError> {
    let pretty = serde_json::to_string_pretty(value).map_err(FingerprintError::from)?;
    Ok(ensure_ascii(&pretty) + "\n")
}

pub const BASELINE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct BaselineEntry {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct BaselineFile {
    pub version: u32,
    pub entries: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BaselineError {
    #[error("baseline JSON parses: {detail}")]
    Json { detail: String },
    #[error("unsupported baseline version {found}: want 1")]
    UnsupportedVersion { found: u32 },
    #[error("baseline entry {index} has empty tool: want a non-empty tool identity")]
    EmptyTool { index: usize },
    #[error("baseline entry {index} has empty message: want a non-empty message")]
    EmptyMessage { index: usize },
    #[error("baseline entry {index} has empty context: want a non-empty context digest")]
    EmptyContext { index: usize },
    #[error("baseline entry {index} has empty rule: omit the rule instead of an empty string")]
    EmptyRule { index: usize },
    #[error("baseline entry {index} has empty path: omit the path instead of an empty string")]
    EmptyPath { index: usize },
    #[error("invalid path at baseline entry {index} {path:?}: {reason}")]
    BadPath {
        index: usize,
        path: String,
        reason: &'static str,
    },
}

impl From<serde_json::Error> for BaselineError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json {
            detail: error.to_string(),
        }
    }
}

fn baseline_path_reason(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return Some("path must be non-empty");
    }
    let bytes = path.as_bytes();
    if path.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return Some("path must be workspace-relative, not absolute");
    }
    if path.contains('\\') {
        return Some("path must use forward slashes");
    }
    if path.split('/').any(|component| component == ".") {
        return Some("path must have no '.' component");
    }
    if path.split('/').any(|component| component == "..") {
        return Some("path must have no '..' component");
    }
    if path.chars().any(char::is_control) {
        return Some("path must have no control character");
    }
    if path.split('/').any(str::is_empty) {
        return Some("path must have no empty component");
    }
    None
}

pub fn normalize_message(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn normalize_path(path: &str) -> String {
    let mut trimmed = path.trim().to_owned();
    while let Some(rest) = trimmed.strip_prefix("./") {
        trimmed = rest.to_owned();
    }
    trimmed
}

pub fn fingerprint_of(
    tool: &str,
    rule: Option<&str>,
    path: Option<&str>,
    message: &str,
    context: &str,
) -> String {
    let rule = rule.unwrap_or_default();
    let path = path.unwrap_or_default();
    format!(
        "{}\0{}\0{}\0{}\0{}",
        tool,
        rule,
        normalize_path(path),
        normalize_message(message),
        context
    )
}

pub fn baseline_fingerprint(entry: &BaselineEntry) -> String {
    fingerprint_of(
        &entry.tool,
        entry.rule.as_deref(),
        entry.path.as_deref(),
        &entry.message,
        &entry.context,
    )
}

pub fn parse_baseline(bytes: &[u8]) -> Result<BaselineFile, BaselineError> {
    let file: BaselineFile = serde_json::from_slice(bytes)?;
    if file.version != BASELINE_SCHEMA_VERSION {
        return Err(BaselineError::UnsupportedVersion {
            found: file.version,
        });
    }
    for (index, entry) in file.entries.iter().enumerate() {
        if entry.tool.is_empty() {
            return Err(BaselineError::EmptyTool { index });
        }
        if entry.message.is_empty() {
            return Err(BaselineError::EmptyMessage { index });
        }
        if entry.context.is_empty() {
            return Err(BaselineError::EmptyContext { index });
        }
        if entry.rule.as_deref().is_some_and(str::is_empty) {
            return Err(BaselineError::EmptyRule { index });
        }
        if entry.path.as_deref().is_some_and(str::is_empty) {
            return Err(BaselineError::EmptyPath { index });
        }
        if let Some(path) = entry.path.as_deref() {
            if let Some(reason) = baseline_path_reason(path) {
                return Err(BaselineError::BadPath {
                    index,
                    path: path.to_owned(),
                    reason,
                });
            }
        }
    }
    Ok(file)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineMatch {
    pub suppressed: Vec<usize>,
    pub new: Vec<usize>,
    pub stale: Vec<usize>,
}

pub fn match_baseline(current: &[String], baseline: &BaselineFile) -> BaselineMatch {
    use std::collections::{BTreeMap, VecDeque};
    let mut available: BTreeMap<String, VecDeque<usize>> = BTreeMap::new();
    for (index, entry) in baseline.entries.iter().enumerate() {
        available
            .entry(baseline_fingerprint(entry))
            .or_default()
            .push_back(index);
    }
    let mut suppressed = Vec::new();
    let mut fresh = Vec::new();
    let mut consumed = vec![false; baseline.entries.len()];
    for (current_index, fingerprint) in current.iter().enumerate() {
        match available.get_mut(fingerprint) {
            Some(queue) => match queue.pop_front() {
                Some(baseline_index) => {
                    consumed[baseline_index] = true;
                    suppressed.push(current_index);
                }
                None => fresh.push(current_index),
            },
            None => fresh.push(current_index),
        }
    }
    let mut stale = Vec::new();
    for (index, used) in consumed.iter().enumerate() {
        if !used {
            stale.push(index);
        }
    }
    BaselineMatch {
        suppressed,
        new: fresh,
        stale,
    }
}

pub fn render_baseline(file: &BaselineFile) -> Result<String, BaselineError> {
    let value = serde_json::to_value(file)?;
    to_json_ascii_pretty(&value).map_err(|error| BaselineError::Json {
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_string_bool_vec_shapes() {
        #[derive(serde::Serialize)]
        struct View<'a> {
            name: &'a str,
            read_only: bool,
            tags: Vec<&'a str>,
        }
        let view = View {
            name: "a\"b",
            read_only: true,
            tags: vec!["x", "y"],
        };
        assert_eq!(
            to_json(&view).expect("fingerprint renders"),
            r#"{"name":"a\"b","read_only":true,"tags":["x","y"]}"#
        );
    }

    #[test]
    fn ascii_escape_matches_python_ensure_ascii() {
        assert_eq!(ensure_ascii("caf\u{e9}"), "caf\\u00e9");
        assert_eq!(ensure_ascii("a\u{1F600}b"), "a\\ud83d\\ude00b");
        assert_eq!(ensure_ascii("plain"), "plain");
        let value = serde_json::json!({"name": "caf\u{e9}"});
        let text = to_json_ascii_pretty(&value).expect("pretty ascii");
        assert!(text.contains("caf\\u00e9"));
        assert!(!text.contains("caf\u{e9}"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn json_error_is_typed_with_display() {
        use std::collections::HashMap;
        let mut bad: HashMap<Vec<u8>, String> = HashMap::new();
        bad.insert(vec![0xff], "x".to_owned());
        let err = to_json(&bad).expect_err("non-string keys must fail");
        assert!(err.to_string().contains("fingerprint JSON serializes"));
    }

    fn baseline_entry(tool: &str, path: &str, message: &str, context: &str) -> BaselineEntry {
        BaselineEntry {
            tool: tool.to_owned(),
            rule: Some(format!("{tool}/rule")),
            path: Some(path.to_owned()),
            message: message.to_owned(),
            context: context.to_owned(),
        }
    }

    #[test]
    fn baseline_normalizes_message_and_path_for_identity() {
        assert_eq!(normalize_message("  a   b\nc  "), "a b c");
        assert_eq!(normalize_path("./src/a.py"), "src/a.py");
        assert_eq!(normalize_path("src/a.py"), "src/a.py");
        let first = fingerprint_of(
            "ruff",
            Some("F401"),
            Some("src/a.py"),
            "Imported  but unused",
            "ctx",
        );
        let second = fingerprint_of(
            "ruff",
            Some("F401"),
            Some("src/a.py"),
            "Imported but unused",
            "ctx",
        );
        assert_eq!(first, second);
    }

    #[test]
    fn baseline_location_offsets_alone_never_change_identity() {
        let first = fingerprint_of(
            "ruff",
            Some("F401"),
            Some("src/a.py"),
            "unused",
            "snippet",
        );
        let second = fingerprint_of(
            "ruff",
            Some("F401"),
            Some("src/a.py"),
            "unused",
            "snippet",
        );
        assert_eq!(first, second);
    }

    #[test]
    fn baseline_multiset_limits_suppression_to_recorded_counts() {
        let file = BaselineFile {
            version: BASELINE_SCHEMA_VERSION,
            entries: vec![baseline_entry("ruff", "src/a.py", "unused", "c1")],
        };
        let current = vec![
            fingerprint_of(
                "ruff",
                Some("ruff/rule"),
                Some("src/a.py"),
                "unused",
                "c1",
            ),
            fingerprint_of(
                "ruff",
                Some("ruff/rule"),
                Some("src/a.py"),
                "unused",
                "c1",
            ),
        ];
        let matched = match_baseline(&current, &file);
        assert_eq!(matched.suppressed, vec![0]);
        assert_eq!(matched.new, vec![1]);
        assert!(matched.stale.is_empty());
    }

    #[test]
    fn baseline_rename_and_rule_change_need_refresh() {
        let file = BaselineFile {
            version: BASELINE_SCHEMA_VERSION,
            entries: vec![baseline_entry("ruff", "src/a.py", "unused", "c1")],
        };
        let renamed = vec![fingerprint_of(
            "ruff",
            Some("ruff/rule"),
            Some("src/b.py"),
            "unused",
            "c1",
        )];
        let matched = match_baseline(&renamed, &file);
        assert_eq!(matched.suppressed, Vec::<usize>::new());
        assert_eq!(matched.new, vec![0]);
        assert_eq!(matched.stale, vec![0]);
        let retooled = vec![fingerprint_of(
            "ruff",
            Some("ruff/other"),
            Some("src/a.py"),
            "unused",
            "c1",
        )];
        let matched = match_baseline(&retooled, &file);
        assert_eq!(matched.new, vec![0]);
        assert_eq!(matched.stale, vec![0]);
    }

    #[test]
    fn baseline_roundtrip_and_malformed_entries_fail() {
        let file = BaselineFile {
            version: BASELINE_SCHEMA_VERSION,
            entries: vec![baseline_entry("ruff", "src/a.py", "unused", "c1")],
        };
        let text = render_baseline(&file).expect("render");
        assert!(text.ends_with('\n'));
        let parsed = parse_baseline(text.as_bytes()).expect("parse");
        assert_eq!(parsed, file);
        assert!(parse_baseline(b"{not json").is_err());
        let mut wrong = file.clone();
        wrong.version = 2;
        let bytes = serde_json::to_vec(&wrong).expect("json");
        assert_eq!(
            parse_baseline(&bytes),
            Err(BaselineError::UnsupportedVersion { found: 2 })
        );
        let mut empty_tool = file.clone();
        empty_tool.entries[0].tool.clear();
        let bytes = serde_json::to_vec(&empty_tool).expect("json");
        assert_eq!(
            parse_baseline(&bytes),
            Err(BaselineError::EmptyTool { index: 0 })
        );
        let mut empty_rule = file.clone();
        empty_rule.entries[0].rule = Some(String::new());
        let bytes = serde_json::to_vec(&empty_rule).expect("json");
        assert_eq!(
            parse_baseline(&bytes),
            Err(BaselineError::EmptyRule { index: 0 })
        );
        let mut bad_path = file.clone();
        bad_path.entries[0].path = Some("/absolute".to_owned());
        let bytes = serde_json::to_vec(&bad_path).expect("json");
        assert!(matches!(
            parse_baseline(&bytes),
            Err(BaselineError::BadPath { index: 0, .. })
        ));
    }
}
