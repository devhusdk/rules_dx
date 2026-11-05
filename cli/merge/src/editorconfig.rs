//! Sets keys in an `.editorconfig` section and keeps every other byte.

fn header(line: &str) -> Option<&str> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    Some(inner)
}

fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with(['#', ';', '[']) {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    Some(key.trim_end())
}

fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Sets each key in the last `[section]`, adding the key or the section when missing.
pub fn set_keys(text: &str, section: &str, keys: &[(&str, &str)]) -> String {
    let eol = line_ending(text);
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    let start = lines.iter().rposition(|line| header(line) == Some(section));
    let Some(start) = start else {
        let mut out = text.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(eol);
        }
        if !out.is_empty() {
            out.push_str(eol);
        }
        out.push_str(&format!("[{section}]{eol}"));
        for (key, value) in keys {
            out.push_str(&format!("{key} = {value}{eol}"));
        }
        return out;
    };
    for (key, value) in keys {
        let end = lines[start + 1..]
            .iter()
            .position(|line| header(line).is_some())
            .map_or(lines.len(), |offset| start + 1 + offset);
        let found = (start + 1..end).find(|&i| {
            key_of(&lines[i]).is_some_and(|existing| existing.eq_ignore_ascii_case(key))
        });
        if let Some(i) = found {
            let line = &lines[i];
            let body = line.trim_end_matches(['\r', '\n']);
            let ending = &line[body.len()..];
            if let Some(eq) = body.find('=') {
                let after = &body[eq + 1..];
                let spacing = &after[..after.len() - after.trim_start().len()];
                lines[i] = format!("{}{}{}{}", &body[..=eq], spacing, value, ending);
            }
            continue;
        }
        let last_key = (start + 1..end)
            .rev()
            .find(|&i| key_of(&lines[i]).is_some());
        let at = last_key.map_or(start + 1, |i| i + 1);
        if at > 0 && !lines[at - 1].ends_with('\n') {
            lines[at - 1].push_str(eol);
        }
        lines.insert(at, format!("{key} = {value}{eol}"));
    }
    lines.concat()
}

#[cfg(test)]
mod tests {
    use super::set_keys;

    #[test]
    fn replaces_only_the_value() {
        let text = "root = true\n\n[*.kt]\n# note\nindent_size   =  2\nother = 1\n";
        let out = set_keys(text, "*.kt", &[("INDENT_SIZE", "4")]);
        assert_eq!(
            out,
            "root = true\n\n[*.kt]\n# note\nindent_size   =  4\nother = 1\n"
        );
    }

    #[test]
    fn adds_missing_key_after_last_key() {
        let text = "[*.kt]\na = 1\n# tail\n\n[*.go]\nb = 2\n";
        let out = set_keys(text, "*.kt", &[("c", "3")]);
        assert_eq!(out, "[*.kt]\na = 1\nc = 3\n# tail\n\n[*.go]\nb = 2\n");
    }

    #[test]
    fn appends_missing_section() {
        let out = set_keys("root = true", "*.kt", &[("a", "1")]);
        assert_eq!(out, "root = true\n\n[*.kt]\na = 1\n");
    }

    #[test]
    fn edits_the_last_duplicate_section() {
        let text = "[x]\na = 1\n[x]\na = 1\n";
        assert_eq!(
            set_keys(text, "x", &[("a", "2")]),
            "[x]\na = 1\n[x]\na = 2\n"
        );
    }

    #[test]
    fn keeps_crlf() {
        let text = "[x]\r\na = 1\r\n";
        assert_eq!(
            set_keys(text, "x", &[("a", "2"), ("b", "3")]),
            "[x]\r\na = 2\r\nb = 3\r\n"
        );
    }

    #[test]
    fn keeps_comments_and_second_run_is_stable() {
        let text = "; c\n[x]\n# k = v\na = 1\n";
        let once = set_keys(text, "x", &[("k", "w")]);
        assert_eq!(once, "; c\n[x]\n# k = v\na = 1\nk = w\n");
        assert_eq!(set_keys(&once, "x", &[("k", "w")]), once);
    }
}
