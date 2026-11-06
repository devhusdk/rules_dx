//! The `advisory` update set: advisory snapshot pins in `.dx/pins` against the newest published release.

use std::collections::BTreeMap;
use std::fmt::Write;

/// The selector `dx update advisory` takes.
pub const SET: &str = "advisory";

/// Branch of the advisory bump pull request.
pub const BRANCH: &str = "dx-update/advisory";

/// Where the newest release of the advisory repository is read.
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/devhusdk/rules_dx-advisory/releases/latest";

/// One `advisory <family> <date> <sha256> <url>` line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pin {
    pub family: String,
    pub date: String,
    pub sha256: String,
    pub url: String,
}

impl Pin {
    pub fn line(&self) -> String {
        format!(
            "advisory {} {} {} {}",
            self.family, self.date, self.sha256, self.url
        )
    }
}

/// Reads one pin line; any other line gives `None`.
pub fn parse_line(line: &str) -> Option<Pin> {
    let fields: Vec<&str> = line.trim_end_matches('\r').split_whitespace().collect();
    let ["advisory", family, date, sha256, url] = fields[..] else {
        return None;
    };
    Some(Pin {
        family: family.to_owned(),
        date: date.to_owned(),
        sha256: sha256.to_owned(),
        url: url.to_owned(),
    })
}

/// Every advisory pin of a `.dx/pins` text, in file order.
pub fn pins(text: &str) -> Vec<Pin> {
    text.lines().filter_map(parse_line).collect()
}

/// The pins one release lists in its body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    pub tag: String,
    pub pins: Vec<Pin>,
}

/// Reads the releases API answer: the tag and the pin lines of the body.
pub fn parse_release(json: &str) -> Result<Release, String> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("invalid release answer: {error}"))?;
    let tag = value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .ok_or("the release has no tag_name")?;
    let body = value
        .get("body")
        .and_then(|body| body.as_str())
        .unwrap_or("");
    let pins = pins(body);
    if pins.is_empty() {
        return Err(format!("release {tag} lists no advisory pin lines"));
    }
    Ok(Release {
        tag: tag.to_owned(),
        pins,
    })
}

/// Replaces the advisory lines of `text` the release has a pin for; every other line stays as it is.
pub fn rewrite(text: &str, release: &Release) -> String {
    let newest: BTreeMap<&str, &Pin> = release
        .pins
        .iter()
        .map(|pin| (pin.family.as_str(), pin))
        .collect();
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let ending = &line[body.len()..];
        match parse_line(body).and_then(|pin| newest.get(pin.family.as_str())) {
            Some(pin) => {
                out.push_str(&pin.line());
                out.push_str(ending);
            }
            None => out.push_str(line),
        }
    }
    out
}

/// Whole days between two `YYYY-MM-DD` dates.
pub fn days_between(from: &str, to: &str) -> Option<i64> {
    let parse = |value: &str| {
        let mut parts = value.split('-').map(|part| part.parse::<i64>().ok());
        let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some((year, month, day))
    };
    Some(day_number(parse(to)?) - day_number(parse(from)?))
}

fn day_number((year, month, day): (i64, i64, i64)) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let shifted = (month + 9) % 12;
    let doy = (153 * shifted + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe
}

/// The status lines of `dx update advisory`.
pub fn status(current: &[Pin], release: &Release, today: &str) -> Vec<String> {
    if current.is_empty() {
        return vec![
            "advisory: .dx/pins has no advisory lines; pin them with advisory_prep --pin"
                .to_owned(),
        ];
    }
    let newest = release.pins.first().map_or("", |pin| pin.date.as_str());
    let mut lines = Vec::new();
    for pin in current {
        let age = days_between(&pin.date, today)
            .map_or_else(String::new, |days| format!(", {days} days old"));
        let behind = match release.pins.iter().find(|new| new.family == pin.family) {
            Some(new) if new.sha256 == pin.sha256 => "current".to_owned(),
            Some(new) => format!("newer: {}", new.date),
            None => "not in the newest release".to_owned(),
        };
        lines.push(format!(
            "advisory:{} {}{age} ({behind})",
            pin.family, pin.date
        ));
    }
    lines.push(format!(
        "advisory: newest release {} ({newest})",
        release.tag
    ));
    lines
}

/// The pin diff of a bump as unified-looking `-`/`+` lines; empty when nothing changes.
pub fn diff(before: &str, after: &str) -> Vec<String> {
    let old = pins(before);
    let new = pins(after);
    let mut lines = Vec::new();
    for pin in &old {
        if !new.contains(pin) {
            lines.push(format!("- {}", pin.line()));
        }
    }
    for pin in &new {
        if !old.contains(pin) {
            lines.push(format!("+ {}", pin.line()));
        }
    }
    lines
}

/// The pull request body: the pin diff and the advisories only the new snapshots report.
pub fn pr_body(release: &Release, diff: &[String], new_findings: &[String]) -> String {
    let mut text = format!(
        "Moves the advisory pins in `.dx/pins` to release {}.\n\n",
        release.tag
    );
    text.push_str("```diff\n");
    for line in diff {
        let _ = writeln!(text, "{line}");
    }
    text.push_str("```\n\n");
    if new_findings.is_empty() {
        text.push_str("`dx security` reports no advisory that the old snapshots did not.\n");
    } else {
        let _ = writeln!(
            text,
            "New advisories in `dx security` ({}):\n",
            new_findings.len()
        );
        for finding in new_findings {
            let _ = writeln!(text, "- {finding}");
        }
    }
    text
}

/// Findings in `after` that `before` does not have.
pub fn new_findings(before: &[String], after: &[String]) -> Vec<String> {
    let mut found: Vec<String> = after
        .iter()
        .filter(|line| !before.contains(line))
        .cloned()
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn pin_line(family: &str, date: &str, sha: &str) -> String {
        format!("advisory {family} {date} {sha} https://example.test/{date}/{family}.json")
    }

    fn release() -> Release {
        let body = format!(
            "Pins\n{}\n{}\n",
            pin_line("cargo", "2026-11-05", SHA_B),
            pin_line("npm", "2026-11-05", SHA_B)
        );
        let json = serde_json::json!({"tag_name": "2026-11-05", "body": body}).to_string();
        parse_release(&json).expect("release")
    }

    #[test]
    fn rewrite_changes_only_the_advisory_lines_with_a_new_pin() {
        let text = format!(
            "0.0.0\n1.2.3 {SHA_A} dx-linux\n{}\r\n{}\nadvisory go 2026-10-01 {SHA_A} https://example.test/go.json\n",
            pin_line("cargo", "2026-10-01", SHA_A),
            pin_line("npm", "2026-10-01", SHA_A)
        );
        let out = rewrite(&text, &release());
        let expected = format!(
            "0.0.0\n1.2.3 {SHA_A} dx-linux\n{}\r\n{}\nadvisory go 2026-10-01 {SHA_A} https://example.test/go.json\n",
            pin_line("cargo", "2026-11-05", SHA_B),
            pin_line("npm", "2026-11-05", SHA_B)
        );
        assert_eq!(out, expected);
        assert_eq!(rewrite(&out, &release()), out);
    }

    #[test]
    fn release_without_pin_lines_is_an_error() {
        let json = serde_json::json!({"tag_name": "2026-11-05", "body": "nothing"}).to_string();
        assert!(parse_release(&json).is_err());
        assert!(parse_release("{").is_err());
    }

    #[test]
    fn days_between_counts_calendar_days() {
        assert_eq!(days_between("2026-10-31", "2026-11-05"), Some(5));
        assert_eq!(days_between("2026-02-28", "2026-03-01"), Some(1));
        assert_eq!(days_between("2026-13", "2026-03-01"), None);
    }

    #[test]
    fn status_reports_age_and_newest_release() {
        let current = pins(&format!(
            "{}\n{}\n",
            pin_line("cargo", "2026-10-01", SHA_A),
            pin_line("npm", "2026-11-05", SHA_B)
        ));
        let lines = status(&current, &release(), "2026-11-06");
        assert_eq!(
            lines[0],
            "advisory:cargo 2026-10-01, 36 days old (newer: 2026-11-05)"
        );
        assert_eq!(lines[1], "advisory:npm 2026-11-05, 1 days old (current)");
        assert_eq!(lines[2], "advisory: newest release 2026-11-05 (2026-11-05)");
    }

    #[test]
    fn diff_lists_removed_and_added_pins() {
        let before = format!("{}\n", pin_line("cargo", "2026-10-01", SHA_A));
        let after = rewrite(&before, &release());
        assert_eq!(
            diff(&before, &after),
            vec![
                format!("- {}", pin_line("cargo", "2026-10-01", SHA_A)),
                format!("+ {}", pin_line("cargo", "2026-11-05", SHA_B)),
            ]
        );
        assert!(diff(&after, &after).is_empty());
    }

    #[test]
    fn body_lists_only_new_findings() {
        let before = vec!["a@1 vulnerable to X".to_owned()];
        let after = vec![
            "a@1 vulnerable to X".to_owned(),
            "b@2 vulnerable to Y".to_owned(),
        ];
        let found = new_findings(&before, &after);
        assert_eq!(found, vec!["b@2 vulnerable to Y".to_owned()]);
        let text = pr_body(&release(), &["+ line".to_owned()], &found);
        assert!(text.contains("release 2026-11-05"));
        assert!(text.contains("- b@2 vulnerable to Y"));
        assert!(!text.contains("- a@1"));
        assert!(
            pr_body(&release(), &[], &[]).contains("no advisory that the old snapshots did not")
        );
    }
}
