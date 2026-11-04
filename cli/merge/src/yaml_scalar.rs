//! One-line edits of YAML scalar values.

use crate::Error;

/// The text with the value of the first `key:` line replaced; indent, quotes, comment and line break stay.
pub fn set_value(text: &str, key: &str, value: &str) -> Result<String, Error> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some(range) = value_range(line, key) {
            let (from, to) = (offset + range.0, offset + range.1);
            return Ok(format!("{}{}{}", &text[..from], value, &text[to..]));
        }
        offset += line.len();
    }
    Err(Error::MissingKey(key.to_owned()))
}

/// The text with the `@<ref>` of every `uses:` line for `action` replaced.
pub fn set_uses_ref(text: &str, action: &str, reference: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        out.push_str(&uses_line(line, action, reference));
    }
    out
}

fn uses_line(line: &str, action: &str, reference: &str) -> String {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed
        .strip_prefix("uses:")
        .or_else(|| trimmed.strip_prefix("- uses:"))
    else {
        return line.to_owned();
    };
    let Some(at) = rest.find(&format!("{action}@")) else {
        return line.to_owned();
    };
    let from = line.len() - rest.len() + at + action.len() + 1;
    let tail = &line[from..];
    let len = tail
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '#')
        .unwrap_or(tail.len());
    format!("{}{}{}", &line[..from], reference, &line[from + len..])
}

fn value_range(line: &str, key: &str) -> Option<(usize, usize)> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    let rest = rest.strip_prefix(key)?.strip_prefix(':')?;
    let after_colon = line.len() - rest.len();
    let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let start = after_colon + spaces;
    let body = &line[start..];
    let (from, len) = match body.chars().next()? {
        quote @ ('"' | '\'') => (start + 1, body[1..].find(quote)?),
        '\r' | '\n' | '#' => return None,
        _ => (
            start,
            body.find(|c: char| c.is_whitespace()).unwrap_or(body.len()),
        ),
    };
    Some((from, from + len))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CI: &str = "jobs:\r\n  dx:\r\n    uses: devhusdk/rules_dx/.github/workflows/reusable-consumer.yml@aaaa # pin\r\n    with:\r\n      rules_dx_version: \"1.0.0\"\r\n      other: 'x'\r\n";

    #[test]
    fn a_pin_update_changes_exactly_two_lines() {
        let step = set_uses_ref(
            CI,
            "devhusdk/rules_dx/.github/workflows/reusable-consumer.yml",
            "bbbb",
        );
        let out = set_value(&step, "rules_dx_version", "2.0.0").unwrap();
        let before: Vec<_> = CI.split("\r\n").collect();
        let after: Vec<_> = out.split("\r\n").collect();
        let changed: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
        assert_eq!(changed.len(), 2);
        assert!(out.contains("@bbbb # pin\r\n"));
        assert!(out.contains("rules_dx_version: \"2.0.0\"\r\n"));
    }

    #[test]
    fn setting_the_same_values_is_a_no_op() {
        let out = set_value(CI, "rules_dx_version", "1.0.0").unwrap();
        assert_eq!(out, CI);
        let action = "devhusdk/rules_dx/.github/workflows/reusable-consumer.yml";
        assert_eq!(set_uses_ref(CI, action, "aaaa"), CI);
    }

    #[test]
    fn single_quotes_and_bare_values_keep_their_style() {
        assert_eq!(set_value("a: 'x'\n", "a", "y").unwrap(), "a: 'y'\n");
        assert_eq!(
            set_value("  a:   x  # c\n", "a", "y").unwrap(),
            "  a:   y  # c\n"
        );
    }

    #[test]
    fn a_missing_key_is_an_error() {
        assert_eq!(
            set_value("a: 1\n", "b", "2"),
            Err(Error::MissingKey("b".to_owned()))
        );
    }
}
