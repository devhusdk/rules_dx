use super::{finding, lines, FileFinding, ParseError};
use crate::ToolSeverity;
use crate::{ColumnEncoding, TextPosition};

const TOOL: &str = "fsc";

struct Diagnostic<'a> {
    /// Empty when the compiler named no file.
    file: &'a str,
    line: u64,
    column: u64,
    severity: Option<ToolSeverity>,
    code: &'a str,
    message: &'a str,
}

/// `error FS0001: message`, `warning FS3370: message` or `info FS3370: message`.
fn head(text: &str) -> Option<(Option<ToolSeverity>, &str, &str)> {
    let (word, rest) = text.split_once(' ')?;
    let severity = match word {
        "error" => Some(ToolSeverity::Error),
        "warning" => Some(ToolSeverity::Warning),
        "info" | "informational" => None,
        _ => return None,
    };
    let (code, message) = rest.split_once(':')?;
    let code = code.trim();
    let numbered = code.len() > 2
        && code.starts_with("FS")
        && code[2..].bytes().all(|byte| byte.is_ascii_digit());
    numbered.then_some((severity, code, message.trim()))
}

/// The line and column of a `(l,c)` position, also when it spans a range.
fn position(text: &str) -> Option<(u64, u64)> {
    let inner = text.strip_prefix('(')?;
    let inner = &inner[..inner.find(')')?];
    let mut numbers = inner
        .split(['-', ',', ')', '('])
        .filter(|part| !part.is_empty());
    let line = numbers.next()?.parse().ok()?;
    let column = numbers.next()?.parse().ok()?;
    Some((line, column))
}

fn diagnostic(line: &str) -> Option<Diagnostic<'_>> {
    if let Some((severity, code, message)) = head(line) {
        return Some(Diagnostic {
            file: "",
            line: 0,
            column: 0,
            severity,
            code,
            message,
        });
    }
    // The location ends at the first `): ` that is followed by a diagnostic head.
    let mut from = 0;
    while let Some(at) = line[from..].find("): ") {
        let end = from + at + 1;
        let location = &line[..end];
        if let Some((severity, code, message)) = head(line[end + 1..].trim_start()) {
            let open = location.rfind('(')?;
            // A range may repeat the pair as `(l,c)-(l2,c2)`.
            let open = location[..open]
                .strip_suffix('-')
                .and_then(|before| before.rfind('('))
                .unwrap_or(open);
            let (line_no, column) = position(&location[open..])?;
            return Some(Diagnostic {
                file: &location[..open],
                line: line_no,
                column,
                severity,
                code,
                message,
            });
        }
        from = end;
    }
    None
}

/// fsc separates the parts of a type with control characters; join them with one space.
fn tidy(message: &str) -> String {
    message
        .split(|ch: char| ch.is_control() || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The checked file a diagnostic path names, compared by its trailing components so
/// absolute and execution-root paths match.
fn owned<'a>(path: &str, files: &[&'a str]) -> Option<&'a str> {
    let path = path.replace('\\', "/");
    files
        .iter()
        .filter(|file| {
            path == **file
                || path
                    .strip_suffix(**file)
                    .is_some_and(|head| head.ends_with('/'))
        })
        .max_by_key(|file| file.len())
        .copied()
}

/// Parses `fsc --flaterrors` diagnostics. A located error in a file the target does
/// not own, an error that names no file, and a failing exit without a kept error are
/// all errors, never a pass.
pub fn parse_fsc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let mut findings = Vec::new();
    let mut errors = 0_usize;
    for line in lines(TOOL, output)? {
        let Some(found) = diagnostic(line) else {
            continue;
        };
        let owner = (!found.file.is_empty())
            .then(|| owned(found.file, files))
            .flatten();
        let severity = match found.severity {
            Some(ToolSeverity::Error) => {
                let Some(_) = owner else {
                    return Err(ParseError::shape(
                        TOOL,
                        format!("error {} outside the target: {line}", found.code),
                    ));
                };
                errors += 1;
                ToolSeverity::Error
            }
            Some(severity) => severity,
            None => ToolSeverity::Info,
        };
        let Some(file) = owner else {
            continue;
        };
        findings.push(finding(
            TOOL,
            file,
            found.code.to_owned(),
            tidy(found.message),
            severity,
            TextPosition {
                line: found.line,
                column: found.column,
            },
            ColumnEncoding::Scalars,
        ));
    }
    if code != Some(0) && errors == 0 {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {} with no error diagnostics", super::code_name(code)),
        ));
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "fs/a.fs";

    fn parse(text: &str, code: i32) -> Result<Vec<FileFinding>, ParseError> {
        parse_fsc(text.as_bytes(), Some(code), &[FILE])
    }

    #[test]
    fn reports_errors_warnings_and_infos() {
        let text = "fs/a.fs(3,5): error FS0001: This expression was expected to have type 'int'\r\nfs/a.fs(4,1): warning FS0025: Incomplete matches\nfs/a.fs(5,2): info FS3370: Use of '.[]'\n";
        let found = parse(text, 1).expect("parsed");
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].finding.rule_id, "FS0001");
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 5);
        assert!(matches!(found[0].finding.severity, ToolSeverity::Error));
        assert!(matches!(found[1].finding.severity, ToolSeverity::Warning));
        assert!(matches!(found[2].finding.severity, ToolSeverity::Info));
    }

    #[test]
    fn reads_range_forms() {
        for location in ["(3,5-3,9)", "(3,5-9)", "(3,5)-(3,9)"] {
            let text = format!("fs/a.fs{location}: error FS0001: boom\n");
            let found = parse(&text, 1).expect("parsed");
            assert_eq!(found.len(), 1, "{location}");
            assert_eq!(found[0].finding.start.line, 3, "{location}");
            assert_eq!(found[0].finding.start.column, 5, "{location}");
        }
    }

    #[test]
    fn maps_absolute_and_windows_paths_to_the_owned_file() {
        for path in [
            "/work/exec/root/fs/a.fs",
            "C:\\work\\root\\fs\\a.fs",
            "./fs/a.fs",
        ] {
            let text = format!("{path}(1,1): error FS0001: boom\n");
            let found = parse(&text, 1).expect("parsed");
            assert_eq!(found[0].file, FILE, "{path}");
        }
        assert!(parse("/work/xfs/a.fs(1,1): error FS0001: boom\n", 1).is_err());
    }

    #[test]
    fn a_warning_in_another_file_is_dropped_but_an_error_there_fails() {
        let warning = "other/b.fs(1,1): warning FS0025: x\nfs/a.fs(2,1): error FS0001: y\n";
        assert_eq!(parse(warning, 1).expect("parsed").len(), 1);
        assert!(parse("other/b.fs(1,1): error FS0001: x\n", 1).is_err());
    }

    #[test]
    fn an_unlocated_error_is_an_error() {
        assert!(parse("error FS0225: Source file 'x.fs' could not be found\n", 1).is_err());
        assert!(parse(
            "fs/a.fs(1,1): error FS0001: y\nerror FS0084: assembly missing\n",
            1
        )
        .is_err());
    }

    #[test]
    fn a_failing_exit_without_a_kept_error_is_an_error() {
        assert!(parse("", 1).is_err());
        assert!(parse("fs/a.fs(1,1): warning FS0025: x\n", 1).is_err());
        assert!(parse_fsc(b"", None, &[FILE]).is_err());
        assert!(parse_fsc(&[0xff], Some(1), &[FILE]).is_err());
    }

    #[test]
    fn a_clean_exit_with_no_output_is_clean() {
        assert!(parse("", 0).expect("clean").is_empty());
        assert_eq!(
            parse("fs/a.fs(1,1): warning FS0025: x\n", 0)
                .expect("warned")
                .len(),
            1
        );
    }
}
