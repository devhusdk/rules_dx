use super::{FileFinding, ParseError, code_name, finding, lines};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const SEVERITIES: [(&str, Option<ToolSeverity>); 5] = [
    ("fatal error", Some(ToolSeverity::Error)),
    ("error", Some(ToolSeverity::Error)),
    ("warning", Some(ToolSeverity::Warning)),
    ("note", None),
    ("remark", None),
];

pub(super) fn diagnostic(line: &str) -> Option<(&str, u64, u64, Option<ToolSeverity>, &str)> {
    let (at, word, severity) = SEVERITIES
        .iter()
        .filter_map(|(word, severity)| {
            let marker = format!(": {word}: ");
            line.find(&marker).map(|at| (at, marker.len(), *severity))
        })
        .min_by_key(|(at, _, _)| *at)?;
    let mut parts = line.get(..at)?.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line_no = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    Some((file, line_no, column, severity, line.get(at + word..)?))
}

pub(super) fn include_entry(line: &str) -> Option<(&str, u64)> {
    let rest = line
        .strip_prefix("In file included from ")
        .or_else(|| line.trim_start().strip_prefix("from "))?;
    let rest = rest.trim_end().trim_end_matches([':', ',']);
    let (path, line_no) = rest.rsplit_once(':')?;
    Some((path, line_no.parse().ok()?))
}

pub(super) fn locate<'a>(
    file: &'a str,
    line_no: u64,
    column: u64,
    message: &str,
    included_from: &[(&'a str, u64)],
    files: &[&'a str],
) -> Option<(&'a str, u64, u64, String)> {
    let file = file.strip_prefix("./").unwrap_or(file);
    if let Some(owned) = files.iter().copied().find(|f| *f == file) {
        return Some((owned, line_no, column, message.to_owned()));
    }
    let (outer, outer_line) = included_from.iter().find_map(|(path, at)| {
        let path = path.strip_prefix("./").unwrap_or(path);
        files.iter().copied().find(|f| *f == path).map(|f| (f, *at))
    })?;
    Some((
        outer,
        outer_line,
        1,
        format!("{file}:{line_no}:{column}: {message}"),
    ))
}

pub(super) fn split_rule(message: &str) -> (String, String) {
    match message
        .strip_suffix(']')
        .and_then(|inner| inner.rsplit_once(" ["))
    {
        Some((head, flag)) => (head.trim_end().to_owned(), flag.to_owned()),
        None => (message.trim().to_owned(), String::new()),
    }
}

pub fn parse_clang_syntax(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_syntax";
    let mut findings = Vec::new();
    let mut chain: Vec<(&str, u64)> = Vec::new();
    let text = lines(TOOL, stderr)?;
    for line in text.iter().copied() {
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
        let Some((file, line_no, column, message)) =
            locate(file, line_no, column, message, &included_from, files)
        else {
            continue;
        };
        let (message, rule_id) = split_rule(&message);
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
            ColumnEncoding::Utf8,
        ));
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} with no diagnostics (stderr: {})",
                code_name(code),
                unplaced_stderr(&text)
            ),
        ));
    }
    Ok(findings)
}

fn unplaced_stderr(text: &[&str]) -> String {
    const SHOWN: usize = 20;
    let kept: Vec<&str> = text
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    let shown = kept
        .iter()
        .take(SHOWN)
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    match kept.len() {
        0 => "empty".to_owned(),
        n if n > SHOWN => format!("{shown} | ... {} more lines", n - SHOWN),
        _ => shown,
    }
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
        let err = parse_clang_syntax(
            b"clang: error: no such file or directory: 'x.c'\n",
            Some(1),
            &[FILE],
        )
        .expect_err("driver error");
        assert!(
            err.to_string()
                .contains("exit 1 with no diagnostics (stderr: clang: error: no such file or directory: 'x.c')"),
            "{err}"
        );
        let empty = parse_clang_syntax(b"", Some(1), &[FILE]).expect_err("no output");
        assert!(empty.to_string().contains("(stderr: empty)"), "{empty}");
        let actual = parse_clang_syntax(b"", Some(0), &[FILE]).expect("clean");
        assert!(actual.is_empty(), "{actual:?}");
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

    #[test]
    fn columns_are_utf8_bytes() {
        let line = "const char *s = \"é\"; int b = 1;\n";
        let text = format!("{line}\t{line}");
        let log = format!(
            "{FILE}:1:27: warning: unused variable 'b' [-Wunused-variable]\n{FILE}:2:28: warning: unused variable 'b' [-Wunused-variable]\n"
        );
        let found = parse_clang_syntax(log.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        let offsets = [text.find("b = 1"), text.rfind("b = 1")];
        for (item, offset) in found.iter().zip(offsets) {
            let at = item.finding.start;
            assert_eq!(item.finding.encoding, ColumnEncoding::Utf8);
            assert_eq!(
                crate::line_col_to_byte(&text, at.line, at.column, item.finding.encoding),
                offset.map(|i| i as u64)
            );
        }
    }
}
