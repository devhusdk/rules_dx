//! The six-field NOTICE manifest wire format shared by every producer
//! (`dx license`, `release_tool inventory`) and the release verifier.
//! Depends only on `dx_digest` and `std`.

use dx_error::DxError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeEntry {
    pub package: String,
    pub set: String,
    pub version: String,
    pub license: String,
    pub text_path: String,
    pub text_sha256: String,
}

pub fn parse_notice_manifest(text: &str) -> Result<Vec<NoticeEntry>, DxError> {
    let text = crate::locks::strip_bom(text);
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        let [package, set, version, license, text_path, text_sha256] = fields.as_slice() else {
            return Err(DxError::failure(format!(
                "notice manifest line {}: want 6 '|' fields 'package|set|version|license|text|sha256' where text is a per-package relative path (for example 'cargo/serde/LICENSE'), got {}",
                index + 1,
                fields.len()
            )));
        };
        let entry = NoticeEntry {
            package: package.trim().to_owned(),
            set: set.trim().to_owned(),
            version: version.trim().to_owned(),
            license: license.trim().to_owned(),
            text_path: text_path.trim().to_owned(),
            text_sha256: text_sha256.trim().to_owned(),
        };
        if entry.package.is_empty()
            || entry.set.is_empty()
            || entry.version.is_empty()
            || entry.license.is_empty()
        {
            return Err(DxError::failure(format!(
                "notice manifest line {}: package, set, version, and license must be non-empty",
                index + 1
            )));
        }
        if entry.text_path.is_empty() {
            if !entry.text_sha256.is_empty() {
                return Err(DxError::failure(format!(
                    "notice manifest line {}: sha256 must be empty when no text file is named",
                    index + 1
                )));
            }
        } else if let Err(problem) = notice_text_path_error(&entry.text_path) {
            return Err(DxError::failure(format!(
                "notice manifest line {}: text '{}' {problem}",
                index + 1,
                entry.text_path
            )));
        } else if !dx_digest::is_lower_hex(&entry.text_sha256, 32) {
            return Err(DxError::failure(format!(
                "notice manifest line {}: sha256 for text '{}' must be 64 lowercase hex characters",
                index + 1,
                entry.text_path
            )));
        }
        entries.push(entry);
    }
    entries.sort_by(|a, b| {
        (&a.package, &a.set, &a.version, &a.license)
            .cmp(&(&b.package, &b.set, &b.version, &b.license))
    });
    for pair in entries.windows(2) {
        let [a, b] = pair else {
            continue;
        };
        if (&a.package, &a.set, &a.version, &a.license)
            == (&b.package, &b.set, &b.version, &b.license)
        {
            return Err(DxError::failure(format!(
                "notice manifest: duplicate entry {}@{} ({}) [{}]",
                a.package, a.version, a.license, a.set
            )));
        }
    }
    Ok(entries)
}

/// Checks that a manifest text path is a safe per-package relative path:
/// '/'-separated, no leading '/', no '\\', and no empty, '.', or '..'
/// components. Bare basenames are valid (the shortest suffix).
pub fn notice_text_path_error(text_path: &str) -> Result<(), DxError> {
    if text_path.contains('\\') {
        return Err(DxError::failure("must use '/' separators, not '\\'"));
    }
    if text_path.starts_with('/') {
        return Err(DxError::failure("must be a relative path (no leading '/')"));
    }
    if text_path.contains(['\0', ':']) {
        return Err(DxError::failure("must not contain ':' or NUL"));
    }
    for component in text_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(DxError::failure(
                "must not contain empty, '.', or '..' path components",
            ));
        }
    }
    Ok(())
}

/// The sha256 of the body `render_notice` writes under an entry header
/// (the words with trailing newlines replaced by exactly one `\n`).
#[must_use]
pub fn notice_text_sha256(words: &str) -> String {
    let mut body = words.trim_end_matches('\n').to_owned();
    body.push('\n');
    dx_digest::sha256_hex(body.as_bytes())
}

/// Renders the manifest: `header` as a `#` comment line, then one line per
/// entry sorted by (package, set, version, license).
///
/// # Errors
///
/// Returns an error when the header or any field holds `|`, `\n` or `\r`,
/// because `parse_notice_manifest` could not read that line back.
pub fn render_notice_manifest(header: &str, entries: &[NoticeEntry]) -> Result<String, DxError> {
    if let Some(bad) = field_separator(header) {
        return Err(DxError::failure(format!(
            "notice manifest: header {header:?} must not contain {bad}"
        )));
    }
    let mut sorted: Vec<&NoticeEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        (&a.package, &a.set, &a.version, &a.license)
            .cmp(&(&b.package, &b.set, &b.version, &b.license))
    });
    let mut out = String::new();
    if !header.is_empty() {
        out.push_str("# ");
        out.push_str(header);
        out.push('\n');
    }
    for e in sorted {
        let fields = [
            ("package", &e.package),
            ("set", &e.set),
            ("version", &e.version),
            ("license", &e.license),
            ("text", &e.text_path),
            ("sha256", &e.text_sha256),
        ];
        for (name, value) in fields {
            if let Some(bad) = field_separator(value) {
                return Err(DxError::failure(format!(
                    "notice manifest: {name} {value:?} of {}@{} must not contain {bad}",
                    e.package, e.version
                )));
            }
        }
        out.push_str(&fields.map(|(_, value)| value.as_str()).join("|"));
        out.push('\n');
    }
    Ok(out)
}

/// Names the first manifest separator (`|`, `\n` or `\r`) in `value`, if any.
fn field_separator(value: &str) -> Option<&'static str> {
    if value.contains('|') {
        Some("'|'")
    } else if value.contains('\n') {
        Some("'\\n'")
    } else if value.contains('\r') {
        Some("'\\r'")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(package: &str, words: &str) -> NoticeEntry {
        NoticeEntry {
            package: package.to_owned(),
            set: "cargo".to_owned(),
            version: "1.0.0".to_owned(),
            license: "MIT".to_owned(),
            text_path: format!("cargo/{package}/LICENSE"),
            text_sha256: notice_text_sha256(words),
        }
    }

    #[test]
    fn render_parse_round_trip_is_order_independent() {
        let a = entry("aaa", "A\n");
        let b = entry("bbb", "B\n");
        let one = render_notice_manifest("gen", &[a.clone(), b.clone()]).expect("renders");
        let two = render_notice_manifest("gen", &[b.clone(), a.clone()]).expect("renders");
        assert_eq!(one, two);
        assert!(one.starts_with("# gen\n"));
        assert_eq!(parse_notice_manifest(&one), Ok(vec![a, b]));
    }

    #[test]
    fn render_refuses_separators_in_fields_and_header() {
        for bad in ["a|b", "a\nb", "a\rb"] {
            let mut fields: Vec<NoticeEntry> = Vec::new();
            for index in 0..6 {
                let mut e = entry("aaa", "A\n");
                match index {
                    0 => e.package = bad.to_owned(),
                    1 => e.set = bad.to_owned(),
                    2 => e.version = bad.to_owned(),
                    3 => e.license = bad.to_owned(),
                    4 => e.text_path = bad.to_owned(),
                    _ => e.text_sha256 = bad.to_owned(),
                }
                fields.push(e);
            }
            for e in fields {
                let err = render_notice_manifest("gen", &[e]).expect_err("separator in field");
                assert!(err.message().contains("must not contain"), "{err}");
            }
            let err = render_notice_manifest(bad, &[]).expect_err("separator in header");
            assert!(
                err.message().starts_with("notice manifest: header"),
                "{err}"
            );
        }
    }

    #[test]
    fn manifest_ignores_a_leading_byte_order_mark() {
        let sha = "a".repeat(64);
        let line = format!("pkg|set|1.0|MIT|cargo/pkg/LICENSE|{sha}\n");
        let plain = parse_notice_manifest(&line).expect("plain");
        let with_bom = parse_notice_manifest(&format!("\u{feff}{line}")).expect("bom");
        assert_eq!(plain, with_bom);
        parse_notice_manifest(&format!("\u{feff}# header\n{line}")).expect("bom comment");
    }

    #[test]
    fn text_path_rejects_drive_letters_and_nul() {
        for path in ["C:/x/LICENSE", "C:x/LICENSE", "a/b:stream", "a/\0b"] {
            assert!(notice_text_path_error(path).is_err(), "{path:?}");
        }
        assert!(notice_text_path_error("cargo/serde/LICENSE").is_ok());
    }
}
