use crate::{ColumnEncoding, TextPosition};
use std::path::Path;

use super::{as_text, code_name, finding, missing, unformatted_at, FileFinding, ParseError};
use crate::ToolSeverity;

const TOOL: &str = "djlint";

/// Fails when djlint reports a bad configuration or skipped the file, since both exit 0.
pub fn djlint_ran(stderr: &[u8]) -> Result<(), ParseError> {
    let text = String::from_utf8_lossy(stderr);
    if let Some(line) = text
        .lines()
        .find(|line| line.contains("Failed to load config file"))
    {
        return Err(missing(TOOL, "finding", line));
    }
    if text.lines().any(|line| line.contains("No files to check!")) {
        return Err(ParseError::shape(
            TOOL,
            "the file was skipped by the djlint configuration (exclude or require_pragma)",
        ));
    }
    Ok(())
}

/// Parses `djlint --lint --quiet` findings.
pub fn parse_djlint(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let mut findings = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (name, line_no, column, tail) =
            split_location(line).ok_or_else(|| missing(TOOL, "finding", line))?;
        let (rule, message) = tail
            .split_once(' ')
            .ok_or_else(|| missing(TOOL, "finding", line))?;
        if !is_rule(rule) || message.trim().is_empty() {
            return Err(missing(TOOL, "finding", line));
        }
        findings.push(finding(
            TOOL,
            named(files, name)?,
            rule.to_owned(),
            message.trim().to_owned(),
            ToolSeverity::Warning,
            TextPosition {
                line: line_no,
                column: column + 1,
            },
            ColumnEncoding::Scalars,
        ));
    }
    let want = if findings.is_empty() { 0 } else { 1 };
    if code != Some(want) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {} findings", code_name(code), findings.len()),
        ));
    }
    Ok(findings)
}

/// Parses `djlint --check` output.
pub fn parse_djlint_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut changed: Vec<&str> = Vec::new();
    for pair in lines.windows(2) {
        let name = pair[0].trim();
        if !name.is_empty() && !name.starts_with('\u{2500}') && pair[1].starts_with('\u{2500}') {
            changed.push(name);
        }
    }
    let count = lines
        .iter()
        .rev()
        .find_map(|line| updated_count(line))
        .ok_or_else(|| ParseError::shape(TOOL, "no update count in the output"))?;
    if count != changed.len() {
        return Err(ParseError::shape(
            TOOL,
            format!("{count} files counted but {} named", changed.len()),
        ));
    }
    let want = if count == 0 { 0 } else { 1 };
    if code != Some(want) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with {count} files to update", code_name(code)),
        ));
    }
    let mut findings: Vec<FileFinding> = Vec::new();
    for name in changed {
        let file = named(files, name)?;
        if findings.iter().all(|seen| seen.file != file) {
            findings.push(unformatted_at(TOOL, file, 1, 1, ColumnEncoding::Scalars));
        }
    }
    Ok(findings)
}

fn is_rule(text: &str) -> bool {
    let digits = text.trim_start_matches(|c: char| c.is_ascii_uppercase());
    digits.len() < text.len() && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn split_location(line: &str) -> Option<(&str, u64, u64, &str)> {
    for (index, _) in line.match_indices(':') {
        let rest = &line[index + 1..];
        let mut parts = rest.splitn(3, ':');
        let (Some(row), Some(column), Some(tail)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if let (Ok(row), Ok(column)) = (row.parse(), column.parse()) {
            return Some((&line[..index], row, column, tail.trim()));
        }
    }
    None
}

fn updated_count(line: &str) -> Option<usize> {
    let rest = line.trim().strip_suffix(" updated.")?;
    let rest = rest
        .strip_suffix(" would be")
        .or_else(|| rest.strip_suffix(" was"))
        .or_else(|| rest.strip_suffix(" were"))?;
    let (count, noun) = rest.split_once(' ')?;
    if noun != "file" && noun != "files" {
        return None;
    }
    count.parse().ok()
}

/// The checked file a printed name stands for.
fn named<'a>(files: &[&'a str], name: &str) -> Result<&'a str, ParseError> {
    let unknown = || ParseError::UnknownFile {
        tool: TOOL,
        path: name.to_owned(),
    };
    if name.is_empty() {
        return Err(unknown());
    }
    let wanted = Path::new(name);
    let mut hits = files
        .iter()
        .copied()
        .filter(|file| Path::new(file).ends_with(wanted));
    match (hits.next(), hits.next()) {
        (Some(file), None) => Ok(file),
        _ => Err(unknown()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const LINT_DIRTY: &str = "templates/base.html:1:0: H030 Consider adding a meta description.\ntemplates/page.html:2:24: H013 Img tag should have an alt attribute.\n";
    const FILES: &[&str] = &["/s/templates/base.html", "/s/templates/page.html"];
    const CHECK_DIRTY: &str = "Checking 2/2 files\n\n\ntemplates/page.html\n\u{2500}\u{2500}\u{2500}\n@@ -1,3 +1,3 @@\n\n <div>\n-<p>hi</p>\n+    <p>hi</p>\n </div>\n\n1 file would be updated.\n\n";
    #[test]
    fn a_configuration_that_failed_to_load_or_skipped_the_file_is_an_error() {
        let line = b"Failed to load config file /w/x.djlint.toml. Expected ']' (at line 1)\n";
        assert!(djlint_ran(line).is_err());
        assert!(djlint_ran(b"").is_ok());
        assert!(djlint_ran(b"No files to check!\n").is_err());
    }

    #[test]
    fn lint_reports_rule_message_and_position() {
        let found = parse_djlint(LINT_DIRTY.as_bytes(), Some(1), FILES).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].file, "/s/templates/base.html");
        assert_eq!(found[0].finding.rule_id, "H030");
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.column, 1);
        assert_eq!(found[1].file, "/s/templates/page.html");
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[1].finding.start.line, 2);
        assert!(parse_djlint(b"", Some(0), FILES).expect("clean").is_empty());
    }
    #[test]
    fn lint_rejects_output_outside_the_grammar() {
        assert!(parse_djlint(b"", Some(1), FILES).is_err());
        assert!(parse_djlint(b"", Some(2), FILES).is_err());
        assert!(parse_djlint(LINT_DIRTY.as_bytes(), Some(0), FILES).is_err());
        assert!(parse_djlint(b"other.html:1:1: H001 x\n", Some(1), FILES).is_err());
        assert!(parse_djlint(b"page.html:1:1: lowercase x\n", Some(1), FILES).is_err());
        assert!(parse_djlint(b"page.html:1:1: H001\n", Some(1), FILES).is_err());
        assert!(parse_djlint(b"Error: bad profile\n", Some(2), FILES).is_err());
        assert!(parse_djlint(&[0xff], Some(1), FILES).is_err());
    }
    #[test]
    fn a_name_that_fits_two_files_is_refused() {
        let files = ["/s/a/page.html", "/s/b/page.html"];
        assert!(parse_djlint(b"page.html:1:1: H001 x\n", Some(1), &files).is_err());
        let found = parse_djlint(b"a/page.html:1:1: H001 x\n", Some(1), &files).expect("parsed");
        assert_eq!(found[0].file, "/s/a/page.html");
    }
    #[test]
    fn format_names_each_file_it_would_change() {
        let found = parse_djlint_format(CHECK_DIRTY.as_bytes(), Some(1), FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "/s/templates/page.html");
        assert_eq!(found[0].finding.message, "file is not formatted");
        let clean = b"Checking 2/2 files\n\n\n0 files would be updated.\n\n";
        assert!(parse_djlint_format(clean, Some(0), FILES)
            .expect("clean")
            .is_empty());
    }
    #[test]
    fn format_rejects_output_outside_the_grammar() {
        assert!(parse_djlint_format(b"", Some(0), FILES).is_err());
        assert!(parse_djlint_format(CHECK_DIRTY.as_bytes(), Some(0), FILES).is_err());
        let clean = b"0 files would be updated.\n";
        assert!(parse_djlint_format(clean, Some(1), FILES).is_err());
        let miscounted = CHECK_DIRTY.replace("1 file would", "2 files would");
        assert!(parse_djlint_format(miscounted.as_bytes(), Some(1), FILES).is_err());
        let unknown = CHECK_DIRTY.replace("templates/page.html", "templates/other.html");
        assert!(parse_djlint_format(unknown.as_bytes(), Some(1), FILES).is_err());
        assert!(parse_djlint_format(&[0xff], Some(1), FILES).is_err());
    }
}
