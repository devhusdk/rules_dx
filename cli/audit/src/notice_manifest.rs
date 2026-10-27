//! The six-field NOTICE manifest wire format shared by every producer
//! (`dx license`, `release_tool inventory`) and the release verifier.
//! Depends only on `dx_digest` and `std`.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeEntry {
    pub package: String,
    pub set: String,
    pub version: String,
    pub license: String,
    pub text_path: String,
    pub text_sha256: String,
}

pub fn parse_notice_manifest(text: &str) -> Result<Vec<NoticeEntry>, String> {
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 6 {
            return Err(format!(
                "notice manifest line {}: want 6 '|' fields 'package|set|version|license|text|sha256' where text is a per-package relative path (for example 'cargo/serde/LICENSE'), got {} (5-field manifests predate the text sha256 column; regenerate the inventory)",
                index + 1,
                fields.len()
            ));
        }
        let entry = NoticeEntry {
            package: fields[0].trim().to_owned(),
            set: fields[1].trim().to_owned(),
            version: fields[2].trim().to_owned(),
            license: fields[3].trim().to_owned(),
            text_path: fields[4].trim().to_owned(),
            text_sha256: fields[5].trim().to_owned(),
        };
        if entry.package.is_empty()
            || entry.set.is_empty()
            || entry.version.is_empty()
            || entry.license.is_empty()
        {
            return Err(format!(
                "notice manifest line {}: package, set, version, and license must be non-empty",
                index + 1
            ));
        }
        if entry.text_path.is_empty() {
            if !entry.text_sha256.is_empty() {
                return Err(format!(
                    "notice manifest line {}: sha256 must be empty when no text file is named",
                    index + 1
                ));
            }
        } else if let Err(problem) = notice_text_path_error(&entry.text_path) {
            return Err(format!(
                "notice manifest line {}: text '{}' {problem}",
                index + 1,
                entry.text_path
            ));
        } else if !dx_digest::is_lower_hex(&entry.text_sha256, 32) {
            return Err(format!(
                "notice manifest line {}: sha256 for text '{}' must be 64 lowercase hex characters",
                index + 1,
                entry.text_path
            ));
        }
        entries.push(entry);
    }
    entries.sort_by(|a, b| {
        (&a.package, &a.set, &a.version, &a.license)
            .cmp(&(&b.package, &b.set, &b.version, &b.license))
    });
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if (&a.package, &a.set, &a.version, &a.license)
            == (&b.package, &b.set, &b.version, &b.license)
        {
            return Err(format!(
                "notice manifest: duplicate entry {}@{} ({}) [{}]",
                a.package, a.version, a.license, a.set
            ));
        }
    }
    Ok(entries)
}

/// Checks that a manifest text path is a safe per-package relative path:
/// '/'-separated, no leading '/', no '\\', and no empty, '.', or '..'
/// components. Bare basenames are valid (the shortest suffix).
pub fn notice_text_path_error(text_path: &str) -> Result<(), String> {
    if text_path.contains('\\') {
        return Err("must use '/' separators, not '\\'".to_owned());
    }
    if text_path.starts_with('/') {
        return Err("must be a relative path (no leading '/')".to_owned());
    }
    for component in text_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err("must not contain empty, '.', or '..' path components".to_owned());
        }
    }
    Ok(())
}

/// The sha256 of the body `render_notice` writes under an entry header
/// (the words with trailing newlines replaced by exactly one `\n`).
pub fn notice_text_sha256(words: &str) -> String {
    let mut body = words.trim_end_matches('\n').to_owned();
    body.push('\n');
    dx_digest::sha256_hex(body.as_bytes())
}

/// Renders the manifest: `header` as a `#` comment line, then one line per
/// entry sorted by (package, set, version, license).
pub fn render_notice_manifest(header: &str, entries: &[NoticeEntry]) -> String {
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
        out.push_str(&format!(
            "{}|{}|{}|{}|{}|{}\n",
            e.package, e.set, e.version, e.license, e.text_path, e.text_sha256
        ));
    }
    out
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
        let one = render_notice_manifest("gen", &[a.clone(), b.clone()]);
        let two = render_notice_manifest("gen", &[b.clone(), a.clone()]);
        assert_eq!(one, two);
        assert!(one.starts_with("# gen\n"));
        assert_eq!(parse_notice_manifest(&one), Ok(vec![a, b]));
    }
}
