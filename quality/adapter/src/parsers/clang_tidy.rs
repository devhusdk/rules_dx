use super::clang_syntax::{diagnostic, include_entry, split_rule};
use super::{FileFinding, ParseError, finding, lines, missing, require_findings};
use crate::{ColumnEncoding, TextPosition, ToolSeverity};

const INFO_RULES: &[&str] = &[
    "clang-diagnostic-unused-function",
    "misc-unused-parameters",
    "misc-unused-using-decls",
    "readability-redundant-declaration",
];

fn is_count_trailer(line: &str) -> bool {
    let line = line.trim_end();
    let Some(counts) = line.strip_suffix(" generated.") else {
        return false;
    };
    let mut words = counts.split_whitespace();
    let numbered = |word: Option<&str>| word.is_some_and(|w| w.parse::<u64>().is_ok());
    let kind =
        |word: Option<&str>| matches!(word, Some("warning" | "warnings" | "error" | "errors"));
    numbered(words.next()) && kind(words.next())
}

// clang-tidy prints the main file as an absolute path under the action's execroot.
fn checked_file<'a>(files: &[&'a str], path: &str) -> Option<&'a str> {
    if let Some(file) = files.iter().copied().find(|f| *f == path) {
        return Some(file);
    }
    let absolute = path.starts_with('/') || path.get(1..3).is_some_and(|s| s == ":\\" || s == ":/");
    if !absolute {
        return None;
    }
    let path = path.replace('\\', "/");
    files
        .iter()
        .copied()
        .find(|f| !f.starts_with('/') && path.ends_with(&format!("/{}", f.replace('\\', "/"))))
}

pub fn parse_clang_tidy(
    stderr: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "clang_tidy";
    let mut findings = Vec::new();
    let mut seen = false;
    let mut failed = false;
    for line in lines(TOOL, stderr)? {
        if include_entry(line).is_some() {
            continue;
        }
        if let Some((path, line_no, column, severity, message)) = diagnostic(line) {
            seen = true;
            let Some(severity) = severity else {
                continue;
            };
            let path = path.strip_prefix("./").unwrap_or(path);
            let Some(file) = checked_file(files, path) else {
                return Err(ParseError::UnknownFile {
                    tool: TOOL,
                    path: path.to_owned(),
                });
            };
            let (message, rule_id) = split_rule(message.trim());
            let rule_id = rule_id
                .strip_suffix(",-warnings-as-errors")
                .map(str::to_owned)
                .unwrap_or(rule_id);
            if message.is_empty() {
                return Err(missing(TOOL, "message", line));
            }
            let severity = if INFO_RULES.contains(&rule_id.as_str()) {
                ToolSeverity::Info
            } else {
                severity
            };
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
            continue;
        }
        if line.starts_with("Error while processing ") || line.starts_with("Found compiler error") {
            failed = true;
            continue;
        }
        if is_count_trailer(line)
            || line.starts_with("Suppressed ")
            || line.starts_with("Use -header-filter=")
            || line.starts_with("Use -system-headers")
            || seen
        {
            continue;
        }
        return Err(missing(TOOL, "location", line));
    }
    if failed && findings.is_empty() {
        return Err(missing(
            TOOL,
            "location",
            "clang-tidy failed to process the file",
        ));
    }
    require_findings(TOOL, &findings, code)?;
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "cc/tests/fixtures/clang_tidy/Sample.c:4:3: warning: do not use 'else' after 'return' [readability-else-after-return]\n";

    #[test]
    fn clang_tidy_reports_text_diagnostics() {
        let findings = parse_clang_tidy(
            DIRTY.as_bytes(),
            Some(1),
            &["cc/tests/fixtures/clang_tidy/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "cc/tests/fixtures/clang_tidy/Sample.c");
        assert_eq!(findings[0].finding.rule_id, "readability-else-after-return");
        assert_eq!(
            findings[0].finding.message,
            "do not use 'else' after 'return'"
        );
        let clean = parse_clang_tidy(b"", Some(0), &["cc/tests/fixtures/clang_tidy/Sample.c"])
            .expect("parsed");
        assert!(clean.is_empty());
        assert!(
            parse_clang_tidy(b"", Some(1), &["cc/tests/fixtures/clang_tidy/Sample.c"]).is_err()
        );
        assert!(parse_clang_tidy(DIRTY.as_bytes(), Some(1), &["other.c"]).is_err());
        assert!(parse_clang_tidy(b"nope\n", Some(1), &["x"]).is_err());
        assert!(parse_clang_tidy(&[0xff], Some(1), &["x"]).is_err());
    }

    #[test]
    fn clang_tidy_reports_unused_code_rules_as_info() {
        let text = "a.c:1:14: warning: parameter 'x' is unused [misc-unused-parameters]\na.c:4:12: warning: unused function 'f' [clang-diagnostic-unused-function]\na.c:2:3: warning: do not use 'else' after 'return' [readability-else-after-return]\n";
        let findings = parse_clang_tidy(text.as_bytes(), Some(0), &["a.c"]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
        assert_eq!(findings[1].finding.severity, ToolSeverity::Info);
        assert_eq!(findings[2].finding.severity, ToolSeverity::Warning);
    }

    #[test]
    fn clang_tidy_drops_the_warnings_as_errors_marker_from_the_rule() {
        let text = "a.c:1:14: error: parameter 'x' is unused [misc-unused-parameters,-warnings-as-errors]\n";
        let findings = parse_clang_tidy(text.as_bytes(), Some(1), &["a.c"]).expect("parsed");
        assert_eq!(findings[0].finding.rule_id, "misc-unused-parameters");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Info);
    }

    #[test]
    fn clang_tidy_maps_execroot_paths_to_checked_files() {
        let file = "quality/a.c";
        for path in [
            "/sandbox/1/execroot/_main/quality/a.c",
            "C:\\b\\execroot\\_main\\quality\\a.c",
        ] {
            let text = format!(
                "{path}:3:1: warning: do not use 'else' after 'return' [readability-else-after-return]\n"
            );
            let findings = parse_clang_tidy(text.as_bytes(), Some(1), &[file]).expect("parsed");
            assert_eq!(findings[0].file, file);
        }
        let other = "/x/notquality/a.c:3:1: warning: w [readability-else-after-return]\n";
        assert!(parse_clang_tidy(other.as_bytes(), Some(1), &[file]).is_err());
    }

    #[test]
    fn clang_tidy_reads_drive_rooted_paths() {
        let file = "C:\\tmp\\a.c";
        let findings = parse_clang_tidy("C:\\tmp\\a.c:3:1: warning: do not use 'else' after 'return' [readability-else-after-return]\n".as_bytes(), Some(1), &[file]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, file);
        assert_eq!(findings[0].finding.start.line, 3);
        assert_eq!(findings[0].finding.start.column, 1);
    }

    #[test]
    fn clang_tidy_skips_snippets_notes_and_trailers() {
        let log = format!(
            "1 warning generated.\n{DIRTY}    return 1;\n  ^~~~\ncc/tests/fixtures/clang_tidy/Sample.c:3:3: note: previous return\nSuppressed 3 warnings (3 in non-user code).\nUse -header-filter=.* to display errors from all non-system headers. Use -system-headers to display errors from system headers as well.\n"
        );
        let findings = parse_clang_tidy(
            log.as_bytes(),
            Some(0),
            &["cc/tests/fixtures/clang_tidy/Sample.c"],
        )
        .expect("parsed");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn clang_tidy_fails_closed_on_unknown_files_and_crashes() {
        let files = ["x.c"];
        assert!(matches!(
            parse_clang_tidy(DIRTY.as_bytes(), Some(0), &files),
            Err(ParseError::UnknownFile { .. })
        ));
        assert!(parse_clang_tidy(b"Error while processing x.c.\n", Some(0), &files).is_err());
        assert!(parse_clang_tidy(b"nope\n1 warning generated.\n", Some(0), &files).is_err());
    }

    #[test]
    fn clang_tidy_columns_are_utf8_bytes() {
        let file = "cc/a.c";
        let line = "const char *s = \"é\"; int b = 1;\n";
        let text = format!("{line}\t{line}");
        let log = format!(
            "{file}:1:27: warning: unused variable 'b' [-Wunused-variable]\n{file}:2:28: warning: unused variable 'b' [-Wunused-variable]\n"
        );
        let found = parse_clang_tidy(log.as_bytes(), Some(0), &[file]).expect("parsed");
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
