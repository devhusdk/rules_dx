use super::{finding, lines, require_findings, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const SEVERITIES: [(&str, Option<ToolSeverity>); 5] = [
    ("fatal error", Some(ToolSeverity::Error)),
    ("error", Some(ToolSeverity::Error)),
    ("warning", Some(ToolSeverity::Warning)),
    ("note", None),
    ("remark", None),
];

/// One located diagnostic line: file, line, column, severity (None for notes), message.
pub(super) fn diagnostic(line: &str) -> Option<(&str, u64, u64, Option<ToolSeverity>, &str)> {
    let (at, word, severity) = SEVERITIES
        .iter()
        .filter_map(|(word, severity)| {
            let marker = format!(": {word}: ");
            line.find(&marker).map(|at| (at, marker.len(), *severity))
        })
        .min_by_key(|(at, _, _)| *at)?;
    let mut parts = line[..at].rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line_no = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    Some((file, line_no, column, severity, &line[at + word..]))
}

/// One `In file included from path:line:` (clang) or `from path:line,` (GCC) entry.
pub(super) fn include_entry(line: &str) -> Option<(&str, u64)> {
    let rest = line
        .strip_prefix("In file included from ")
        .or_else(|| line.trim_start().strip_prefix("from "))?;
    let rest = rest.trim_end().trim_end_matches([':', ',']);
    let (path, line_no) = rest.rsplit_once(':')?;
    Some((path, line_no.parse().ok()?))
}

/// Parses `clang -fsyntax-only` (or GCC) text diagnostics. Notes, include stacks and
/// the trailing "N errors generated." summary carry no finding; a failing exit with
/// no located diagnostic is an error.
pub fn parse_clang_syntax(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_syntax";
    let mut findings = Vec::new();
    let mut chain: Vec<(&str, u64)> = Vec::new();
    for line in lines(TOOL, stderr)? {
        if let Some(entry) = include_entry(line) {
            chain.push(entry);
            continue;
        }
        let Some((file, line_no, column, severity, message)) = diagnostic(line) else {
            continue;
        };
        let included_from = std::mem::take(&mut chain);
        let Some(severity) = severity else {
            continue;
        };
        let file = file.strip_prefix("./").unwrap_or(file);
        let mut message_prefix = String::new();
        let (file, line_no, column) = if files.contains(&file) {
            (file, line_no, column)
        } else if let Some((outer, outer_line)) = included_from.iter().find_map(|(path, at)| {
            let path = path.strip_prefix("./").unwrap_or(path);
            files.contains(&path).then_some((path, *at))
        }) {
            // A diagnostic in a header outside the checked set is reported at the
            // include site in the translation unit that pulled it in.
            message_prefix = format!("{file}:{line_no}:{column}: ");
            (outer, outer_line, 1)
        } else {
            // Neither the file nor any includer belongs to this target.
            continue;
        };
        let (message, rule_id) = match message.rfind(" [") {
            Some(start) if message.ends_with(']') => (
                message[..start].trim_end().to_owned(),
                message[start + 2..message.len() - 1].to_owned(),
            ),
            _ => (message.trim().to_owned(), String::new()),
        };
        let message = format!("{message_prefix}{message}");
        findings.push(finding(
            TOOL,
            file,
            rule_id,
            message,
            severity,
            TextPosition {
                line: line_no,
                column,
            },
            ColumnEncoding::Scalars,
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "cc/a.c";

    #[test]
    fn reports_errors_and_warnings_and_skips_notes() {
        let text = "In file included from cc/a.c:1:\ncc/a.c:4:3: error: use of undeclared identifier 'x'\ncc/a.c:2:1: note: declared here\ncc/a.c:5:7: warning: unused variable 'y' [-Wunused-variable]\n2 errors generated.\n";
        let found = parse_clang_syntax(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.message, "use of undeclared identifier 'x'");
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[1].finding.rule_id, "-Wunused-variable");
    }

    #[test]
    fn fatal_include_not_found_is_an_error() {
        let text = "cc/a.c:1:10: fatal error: 'missing.h' file not found\n";
        let found = parse_clang_syntax(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Error));
    }

    #[test]
    fn failing_exit_without_a_diagnostic_is_an_error() {
        assert!(parse_clang_syntax(b"", Some(1), &[FILE]).is_err());
        assert!(parse_clang_syntax(
            b"clang: error: no such file or directory: 'x.c'\n",
            Some(1),
            &[FILE]
        )
        .is_err());
        assert!(parse_clang_syntax(b"", Some(0), &[FILE])
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn a_diagnostic_in_an_unchecked_file_without_includer_is_an_error() {
        let text = "other/b.h:1:1: error: boom\n";
        assert!(parse_clang_syntax(text.as_bytes(), Some(1), &[FILE]).is_err());
        assert!(parse_clang_syntax(&[0xff], Some(1), &[FILE]).is_err());
    }

    #[test]
    fn reads_drive_rooted_paths() {
        let file = "C:\\tmp\\a.c";
        let text = "C:\\tmp\\a.c:3:1: error: boom\n";
        let found = parse_clang_syntax(text.as_bytes(), Some(1), &[file]).expect("parsed");
        assert_eq!(found[0].file, file);
    }

    #[test]
    fn leading_dot_slash_is_normalized() {
        let text = "./cc/a.c:3:1: error: boom\n";
        let found = parse_clang_syntax(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found[0].file, FILE);
    }

    #[test]
    fn header_diagnostics_map_to_the_including_unit() {
        let text = "In file included from cc/a.c:7:\n./dep/b.h:2:5: error: boom\n";
        let found = parse_clang_syntax(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.start.line, 7);
        assert!(found[0].finding.message.starts_with("dep/b.h:2:5: "));
        let gcc = "In file included from dep/b.h:1,\n                 from cc/a.c:7:\ndep/c.h:2:5: warning: w [-Wx]\n";
        let found = parse_clang_syntax(gcc.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found[0].finding.start.line, 7);
    }
}
