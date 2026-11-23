#[must_use]
pub fn set_uses_ref(text: &str, action: &str, reference: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        out.push_str(&uses_line(line, action, reference));
    }
    out
}

fn uses_line(line: &str, action: &str, reference: &str) -> String {
    let mut key = line.trim_start();
    if let Some(item) = key.strip_prefix('-') {
        key = item.trim_start();
    }
    let value = key
        .strip_prefix("uses:")
        .map(str::trim_start)
        .map(|value| value.strip_prefix(['"', '\'']).unwrap_or(value));
    let Some(tail) = value
        .and_then(|value| value.strip_prefix(action))
        .and_then(|rest| rest.strip_prefix('@'))
    else {
        return line.to_owned();
    };
    let Some((head, _)) = line.split_at_checked(line.len() - tail.len()) else {
        return line.to_owned();
    };
    let len = tail
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '#')
        .unwrap_or(tail.len());
    format!(
        "{}{}{}",
        head,
        reference,
        tail.get(len..).unwrap_or_default()
    )
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

    #[test]
    fn a_longer_action_name_is_not_a_match() {
        let text = "    uses: other/actions/checkout@v1\n";
        assert_eq!(set_uses_ref(text, "actions/checkout", "v2"), text);
    }

    #[test]
    fn a_comment_is_not_edited() {
        let text = "    uses: x/y@v1 # was actions/checkout@v0\n";
        assert_eq!(set_uses_ref(text, "actions/checkout", "v2"), text);
    }

    #[test]
    fn extra_spaces_after_the_dash_still_match() {
        let out = set_uses_ref(
            "  -   uses: actions/checkout@v1\n",
            "actions/checkout",
            "v2",
        );
        assert_eq!(out, "  -   uses: actions/checkout@v2\n");
    }

    #[test]
    fn a_quoted_action_is_updated() {
        let out = set_uses_ref("uses: \"actions/checkout@v1\"\n", "actions/checkout", "v2");
        assert_eq!(out, "uses: \"actions/checkout@v2\"\n");
    }
}
