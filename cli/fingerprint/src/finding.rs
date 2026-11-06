#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub target: String,
    pub message: String,
    pub context: Option<[u8; 32]>,
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
}
