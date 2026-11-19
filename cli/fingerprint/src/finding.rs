#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub tool: String,
    pub rule: String,
    pub path: String,
    pub target: String,
    pub message: String,
    pub context: Option<[u8; 32]>,
}

#[must_use]
pub fn normalize_message(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[must_use]
pub fn line_context_digest(file_bytes: &[u8], start_byte: u64) -> Option<[u8; 32]> {
    if file_bytes.is_empty() {
        return None;
    }
    let start: usize = start_byte.try_into().ok()?;
    if start >= file_bytes.len() {
        return None;
    }
    let (head, tail) = file_bytes.split_at_checked(start)?;
    let line_start = head
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    let line_end = start
        + tail
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(tail.len());
    let mut line = file_bytes.get(line_start..line_end)?;
    if line_start == 0 {
        line = line.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(line);
    }
    while let Some(rest) = line.strip_suffix(b"\r") {
        line = rest;
    }
    Some(dx_digest::blake3(line))
}

#[must_use]
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

#[must_use]
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
    let path = path_key(path);
    let path = path.as_str();
    let context = dx_digest::to_hex(context);
    let mut key = String::new();
    for field in [tool, rule, path, message, context.as_str()] {
        push_field(&mut key, field);
    }
    if !target.is_empty() {
        push_field(&mut key, target);
    }
    key
}

fn path_key(path: &str) -> String {
    let path = dx_path::spelling::posix_str(&dx_path::verbatim::without_verbatim_str(path));
    let mut key = if path.starts_with("//") {
        "//".to_owned()
    } else if path.starts_with('/') {
        "/".to_owned()
    } else {
        String::new()
    };
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".");
    for (index, segment) in segments.enumerate() {
        if index > 0 {
            key.push('/');
        }
        if index == 0 && dx_path::drive_prefix(segment) {
            let mut chars = segment.chars();
            key.extend(chars.next().map(|letter| letter.to_ascii_lowercase()));
            key.push_str(chars.as_str());
        } else {
            key.push_str(segment);
        }
    }
    key
}

fn push_field(key: &mut String, field: &str) {
    key.push_str(&field.len().to_string());
    key.push(':');
    key.push_str(field);
    key.push('\0');
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

    #[test]
    fn path_spelling_does_not_change_the_fingerprint() {
        let posix = finding("t", "r", "src/a.rs", "m", b"x");
        let windows = finding("t", "r", "src\\a.rs", "m", b"x");
        let dotted = finding("t", "r", "./src/a.rs", "m", b"x");
        assert_eq!(finding_fingerprint(&posix), finding_fingerprint(&windows));
        assert_eq!(finding_fingerprint(&posix), finding_fingerprint(&dotted));
    }

    #[test]
    fn fields_cannot_bleed_into_each_other() {
        let left = finding("t", "r\0p", "x", "m", b"x");
        let right = finding("t", "r", "p\0x", "m", b"x");
        assert_ne!(finding_fingerprint(&left), finding_fingerprint(&right));
        let mut with_target = finding("t", "r", "p", "m", b"x");
        with_target.target = "u".to_owned();
        let mut in_message = finding("t", "r", "p", "m", b"x");
        in_message.message = format!("m\0{}\0u", dx_digest::to_hex(&dx_digest::blake3(b"x")));
        assert_ne!(
            finding_fingerprint(&with_target),
            finding_fingerprint(&in_message)
        );
    }

    #[test]
    fn redundant_path_segments_do_not_change_the_fingerprint() {
        let plain = finding_fingerprint(&finding("t", "r", "src/a.rs", "m", b"x"));
        for spelling in [
            "././src/a.rs",
            ".//src/a.rs",
            "src//a.rs",
            "src/./a.rs",
            ".\\.\\src\\a.rs",
        ] {
            assert_eq!(
                finding_fingerprint(&finding("t", "r", spelling, "m", b"x")),
                plain,
                "{spelling}"
            );
        }
        assert_ne!(
            finding_fingerprint(&finding("t", "r", "/src/a.rs", "m", b"x")),
            plain
        );
        assert_ne!(
            finding_fingerprint(&finding("t", "r", "src/../a.rs", "m", b"x")),
            finding_fingerprint(&finding("t", "r", "a.rs", "m", b"x"))
        );
    }

    #[test]
    fn windows_drive_and_verbatim_spellings_share_a_fingerprint() {
        let plain = finding_fingerprint(&finding("t", "r", "c:/repo/a.rs", "m", b"x"));
        for spelling in [
            "C:\\repo\\a.rs",
            "\\\\?\\C:\\repo\\a.rs",
            "//?/c:/repo/a.rs",
        ] {
            assert_eq!(
                finding_fingerprint(&finding("t", "r", spelling, "m", b"x")),
                plain,
                "{spelling}"
            );
        }
        let share = finding_fingerprint(&finding("t", "r", "\\\\srv\\share\\a.rs", "m", b"x"));
        assert_eq!(
            finding_fingerprint(&finding(
                "t",
                "r",
                "\\\\?\\UNC\\srv\\share\\a.rs",
                "m",
                b"x"
            )),
            share
        );
        assert_ne!(
            finding_fingerprint(&finding("t", "r", "/srv/share/a.rs", "m", b"x")),
            share
        );
    }

    #[test]
    fn byte_order_mark_does_not_change_the_first_line() {
        let plain = line_context_digest(b"a\nb", 0).expect("plain");
        let marked = line_context_digest(b"\xEF\xBB\xBFa\nb", 3).expect("bom");
        assert_eq!(plain, marked);
        let inner = line_context_digest(b"x\n\xEF\xBB\xBFa", 2).expect("inner");
        assert_ne!(inner, plain);
    }

    #[test]
    fn offset_at_end_of_file_has_no_line() {
        assert!(line_context_digest(b"a\n", 2).is_none());
        assert!(line_context_digest(b"a", 1).is_none());
    }
}
