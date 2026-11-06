use super::{FileFinding, ParseError, finding, lines, require_findings};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "msvc_syntax";

const SEVERITIES: [(&str, Option<ToolSeverity>); 4] = [
    ("fatal error", Some(ToolSeverity::Error)),
    ("error", Some(ToolSeverity::Error)),
    ("warning", Some(ToolSeverity::Warning)),
    ("note", None),
];

struct Diagnostic<'a> {
    file: &'a str,
    position: Option<(u64, u64)>,
    severity: Option<ToolSeverity>,
    code: &'a str,
    message: &'a str,
}

/// Splits `file(12)` or `file(12,5)` into the file and its position.
fn location(head: &str) -> Option<(&str, Option<(u64, u64)>)> {
    let head = head.trim_end();
    let Some(inner) = head.strip_suffix(')') else {
        return Some((head, None));
    };
    let Some((file, numbers)) = inner.rsplit_once('(') else {
        return Some((head, None));
    };
    let (line, column) = numbers.split_once(',').unwrap_or((numbers, "1"));
    Some((file, Some((line.parse().ok()?, column.parse().ok()?))))
}

/// Reads `error C2065: text`, `warning LNK4098: text` or clang-cl's `error: text`.
fn severity_and_code(tail: &str) -> Option<(Option<ToolSeverity>, &str, &str)> {
    let (word, severity) = SEVERITIES.iter().find(|(word, _)| {
        tail.strip_prefix(word)
            .is_some_and(|rest| rest.starts_with(':') || rest.starts_with(' '))
    })?;
    let rest = &tail[word.len()..];
    if let Some(message) = rest.strip_prefix(": ") {
        return Some((*severity, "", message));
    }
    let rest = rest.strip_prefix(' ')?;
    let (code, message) = rest.split_once(':')?;
    let code = code.trim_end();
    let letters = code.chars().take_while(char::is_ascii_uppercase).count();
    let digits = &code[letters..];
    if letters == 0 || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((*severity, code, message.trim_start()))
}

fn diagnostic(line: &str) -> Option<Diagnostic<'_>> {
    let mut from = 0;
    while let Some(offset) = line[from..].find(": ") {
        let at = from + offset;
        from = at + 1;
        let Some((severity, code, message)) = severity_and_code(&line[at + 2..]) else {
            continue;
        };
        let (file, position) = location(&line[..at])?;
        return Some(Diagnostic {
            file,
            position,
            severity,
            code,
            message,
        });
    }
    None
}

fn checked<'a>(file: &str, files: &[&'a str]) -> Option<&'a str> {
    let file = file.replace('\\', "/");
    let file = file.strip_prefix("./").unwrap_or(&file);
    files
        .iter()
        .copied()
        .find(|f| file == *f || file.ends_with(&format!("/{f}")))
}

/// Parses `cl.exe` and `clang-cl` text diagnostics. Notes and continuation lines carry no
/// finding. A located diagnostic in a file outside `files` is dropped. A linker or driver
/// diagnostic with no source location is reported at the first checked file. A failing
/// exit with no finding is an error.
pub fn parse_msvc_syntax(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    for line in lines(TOOL, stdout)? {
        let Some(found) = diagnostic(line) else {
            continue;
        };
        let Some(severity) = found.severity else {
            continue;
        };
        let (file, (line_no, column)) = match found.position {
            Some(position) => {
                let Some(file) = checked(found.file, files) else {
                    continue;
                };
                (file, position)
            }
            None => {
                let Some(file) = files.first().copied() else {
                    continue;
                };
                (file, (1, 1))
            }
        };
        findings.push(finding(
            TOOL,
            file,
            found.code.to_owned(),
            found.message.trim().to_owned(),
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

    const FILE: &str = "src/a.cpp";

    fn parse(text: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_msvc_syntax(text.as_bytes(), Some(code), &[FILE])
    }

    #[test]
    fn reports_an_error_with_line_column_and_code() {
        let text = "a.cpp\nC:\\src\\src\\a.cpp(12,5): error C2065: 'x': undeclared identifier\n";
        let found = parse(text, 2).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.rule_id, "C2065");
        assert_eq!(found[0].finding.message, "'x': undeclared identifier");
        assert_eq!(found[0].finding.start.line, 12);
        assert_eq!(found[0].finding.start.column, 5);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Error));
    }

    #[test]
    fn reports_a_warning_with_only_a_line() {
        let text =
            "src\\a.cpp(7) : warning C4100: 'p': unreferenced parameter [C:\\b\\a.vcxproj]\n";
        let found = parse(text, 0).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "C4100");
        assert_eq!(found[0].finding.start.line, 7);
        assert_eq!(found[0].finding.start.column, 1);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Warning));
    }

    #[test]
    fn reports_a_fatal_error() {
        let text = "src/a.cpp(1): fatal error C1083: Cannot open include file: 'x.h': No such file or directory\n";
        let found = parse(text, 2).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "C1083");
        assert!(
            found[0]
                .finding
                .message
                .starts_with("Cannot open include file: 'x.h'")
        );
    }

    #[test]
    fn clang_cl_diagnostics_have_no_code() {
        let text = "src/a.cpp(3,9): error: use of undeclared identifier 'y'\n";
        let found = parse(text, 1).expect("parsed");
        assert_eq!(found[0].finding.rule_id, "");
        assert_eq!(found[0].finding.message, "use of undeclared identifier 'y'");
    }

    #[test]
    fn notes_and_continuation_lines_are_skipped() {
        let text = "src/a.cpp(4,1): error C2440: 'initializing': cannot convert\nsrc/a.cpp(4,1): note: types pointed to are unrelated\n        with\n        [\n            T=int\n        ]\n";
        let found = parse(text, 2).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.rule_id, "C2440");
    }

    #[test]
    fn a_linker_error_is_reported_at_the_first_file() {
        let text = "a.obj : error LNK2019: unresolved external symbol main\nLINK : fatal error LNK1104: cannot open file 'b.lib'\n";
        let found = parse(text, 2).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.rule_id, "LNK2019");
        assert_eq!(found[1].finding.rule_id, "LNK1104");
        assert_eq!(found[0].file, FILE);
    }

    #[test]
    fn a_foreign_file_is_dropped_and_fails_closed() {
        let text = "C:\\other\\c.h(2): error C2143: syntax error\n";
        assert!(parse(text, 2).is_err());
        assert!(
            parse(&text.replace("error", "warning"), 0)
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn a_malformed_line_is_not_a_diagnostic() {
        let text =
            "src/a.cpp(x,y): error C2065: nope\nsrc/a.cpp(1,2): error 2065: nope\nerror C1: x\n";
        assert!(parse(text, 0).expect("clean").is_empty());
        assert!(parse(text, 2).is_err());
    }

    #[test]
    fn a_tool_failure_without_a_diagnostic_fails_closed() {
        let text = "cl : Command line error D8021 : invalid numeric argument '/Wx'\n";
        assert!(parse(text, 2).is_err());
        assert!(parse("", 0).expect("clean").is_empty());
    }

    #[test]
    fn non_utf8_input_is_an_error() {
        assert!(parse_msvc_syntax(&[0xff], Some(1), &[FILE]).is_err());
    }
}
