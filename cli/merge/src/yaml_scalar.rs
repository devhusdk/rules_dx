//! One-line edits of YAML scalar values.

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

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION: &str = "devhusdk/rules_dx/.github/workflows/reusable-consumer.yml";
    const CI: &str = "jobs:\r\n  dx:\r\n    uses: devhusdk/rules_dx/.github/workflows/reusable-consumer.yml@aaaa # pin\r\n    with:\r\n      other: 'x'\r\n";

    #[test]
    fn a_pin_update_changes_exactly_one_line() {
        let out = set_uses_ref(CI, ACTION, "bbbb");
        let before: Vec<_> = CI.split("\r\n").collect();
        let after: Vec<_> = out.split("\r\n").collect();
        let changed: Vec<_> = before.iter().zip(&after).filter(|(a, b)| a != b).collect();
        assert_eq!(changed.len(), 1);
        assert!(out.contains("@bbbb # pin\r\n"));
    }

    #[test]
    fn setting_the_same_ref_is_a_no_op() {
        assert_eq!(set_uses_ref(CI, ACTION, "aaaa"), CI);
    }
}
