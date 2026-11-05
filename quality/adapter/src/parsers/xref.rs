use crate::{ColumnEncoding, TextPosition, ToolSeverity};

use super::dialyzer::resolve;
use super::{as_text, code_name, finding, FileFinding, ParseError};

const TOOL: &str = "xref";

/// Findings from `dx_xref.escript`: one `file:line: Module:Function/Arity` per exported function no
/// analysed module calls, on exit 0.
///
/// `files` holds the owned workspace-relative source paths. The script names a file as the beam
/// recorded it, so a reported path is resolved to the owned path it ends with; one that matches
/// nothing owned is dropped.
pub fn parse_xref(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let out = as_text(TOOL, stdout)?;
    if code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {}", code_name(code), out.trim()),
        ));
    }
    let mut owned: Vec<&str> = files.to_vec();
    owned.sort_unstable();
    let mut findings = Vec::new();
    for line in out.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            continue;
        }
        let (place, function) = line
            .split_once(": ")
            .ok_or_else(|| ParseError::shape(TOOL, format!("unexpected line: {line}")))?;
        let (file, row) = place
            .rsplit_once(':')
            .and_then(|(file, row)| Some((file, row.parse::<u64>().ok()?)))
            .ok_or_else(|| ParseError::shape(TOOL, format!("unexpected line: {line}")))?;
        let Some(file) = resolve(&owned, file) else {
            continue;
        };
        findings.push(finding(
            TOOL,
            file,
            "unused_export".to_owned(),
            format!("Exported function {function} is not called by any analysed module"),
            ToolSeverity::Warning,
            TextPosition {
                line: row,
                column: 1,
            },
            ColumnEncoding::Scalars,
        ));
    }
    findings.sort_by(|a, b| {
        (&a.file, a.finding.start.line, &a.finding.message).cmp(&(
            &b.file,
            b.finding.start.line,
            &b.finding.message,
        ))
    });
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_reports_unused_exports_of_owned_files_as_info() {
        let out = "/s/src/a.erl:7: a:old/2\nb.erl:1: b:f/0\nsrc/a.erl:3: a:g/0\n";
        let found = parse_xref(out.as_bytes(), Some(0), &["src/a.erl"]).expect("parsed");
        let seen: Vec<_> = found
            .iter()
            .map(|f| (f.file.as_str(), f.finding.start.line))
            .collect();
        assert_eq!(seen, vec![("src/a.erl", 3), ("src/a.erl", 7)]);
        assert_eq!(found[1].finding.rule_id, "unused_export");
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
        assert!(found[1].finding.message.contains("a:old/2"));
    }

    #[test]
    fn xref_clean_run_has_no_findings() {
        assert!(parse_xref(b"", Some(0), &["a.erl"])
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn xref_rejects_a_failed_run_and_stray_lines() {
        assert!(parse_xref(b"boom\n", Some(1), &["a.erl"]).is_err());
        assert!(parse_xref(b"no position\n", Some(0), &["a.erl"]).is_err());
    }
}
