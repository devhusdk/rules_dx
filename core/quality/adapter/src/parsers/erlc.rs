use super::gnu_diagnostics::{Dialect, Disposition, read};
use super::{FileFinding, ParseError};
use crate::ColumnEncoding;
use crate::ToolSeverity;

const DIALECT: Dialect = Dialect {
    tool: "erlc",
    // erlc columns count characters of the line.
    encoding: ColumnEncoding::Scalars,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        "warning" => Some(Disposition::Report(ToolSeverity::Warning)),
        _ => None,
    },
    bracket_rule: false,
};

/// The checked source erlc means by a bare file name, since it prints only the base name.
fn checked_path<'a>(name: &'a str, files: &[&'a str]) -> &'a str {
    if name.contains('/') {
        return name;
    }
    let mut matches = files
        .iter()
        .copied()
        .filter(|file| file.rsplit('/').next() == Some(name));
    match (matches.next(), matches.next()) {
        (Some(file), None) => file,
        _ => name,
    }
}

/// Puts the full source path on every diagnostic and marks those without a severity word as errors.
fn normalized(text: &str, files: &[&str]) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        match located(line) {
            Some((file, position, rest)) => {
                out.push_str(checked_path(file, files));
                out.push_str(position);
                if !rest.starts_with("Warning: ") {
                    out.push_str("Error: ");
                }
                out.push_str(rest);
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

/// Splits `file:line:col: text` or `file:line: text` into the file, the position and the text.
fn located(line: &str) -> Option<(&str, &str, &str)> {
    let (file, rest) = line.split_once(':')?;
    if file.is_empty() || file.contains(' ') {
        return None;
    }
    let (number, rest) = rest.split_once(':')?;
    number.parse::<u64>().ok()?;
    let after_column = match rest.split_once(':') {
        Some((column, after)) if column.parse::<u64>().is_ok() => after,
        _ => rest,
    };
    let text = after_column.strip_prefix(' ')?;
    let position = &line[file.len()..line.len() - text.len()];
    Some((file, position, text))
}

/// `erlc` output. A nonzero exit that printed no diagnostic is an error.
pub fn parse_erlc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let Ok(text) = std::str::from_utf8(output) else {
        return read(&DIALECT, output, code, files);
    };
    read(&DIALECT, normalized(text, files).as_bytes(), code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "src/a.erl";

    #[test]
    fn erlc_reports_unused_variables_and_functions() {
        let text = "src/a.erl:5:9: Warning: variable 'X' is unused\nsrc/a.erl:8:1: Warning: function helper/0 is unused\n";
        let found = parse_erlc(text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 5);
        assert_eq!(found[0].finding.start.column, 9);
        assert_eq!(found[0].finding.message, "variable 'X' is unused");
        assert_eq!(found[1].finding.message, "function helper/0 is unused");
    }

    #[test]
    fn erlc_reports_errors_without_a_severity_word() {
        let text = "src/a.erl:4:10: syntax error before: 'end'\nsrc/a.erl:6:3: function f/0 undefined\n";
        let found = parse_erlc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.message, "syntax error before: 'end'");
        assert_eq!(found[1].finding.message, "function f/0 undefined");
    }

    #[test]
    fn erlc_maps_a_bare_file_name_to_the_checked_source() {
        let text = "a.erl:5:9: Warning: variable 'X' is unused\n";
        let found = parse_erlc(text.as_bytes(), Some(0), &[FILE, "src/b.erl"]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, FILE);
        let twice = parse_erlc(text.as_bytes(), Some(0), &["x/a.erl", "y/a.erl"]).expect("parsed");
        assert!(twice.is_empty());
    }

    #[test]
    fn erlc_accepts_a_missing_column() {
        let text = "src/a.erl:7: Warning: variable 'Y' is unused\n";
        let found = parse_erlc(text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found[0].finding.start.line, 7);
        assert_eq!(found[0].finding.start.column, 1);
    }

    #[test]
    fn erlc_drops_foreign_warnings_and_fails_on_foreign_errors() {
        let warning = "include/b.hrl:1:1: Warning: function g/0 is unused\n";
        assert!(
            parse_erlc(warning.as_bytes(), Some(0), &[FILE])
                .expect("parsed")
                .is_empty()
        );
        let error = "include/b.hrl:1:1: syntax error before: '.'\n";
        assert!(parse_erlc(error.as_bytes(), Some(1), &[FILE]).is_err());
    }

    #[test]
    fn erlc_fails_closed() {
        assert!(parse_erlc(b"", Some(0), &[FILE]).expect("clean").is_empty());
        assert!(parse_erlc(b"", Some(1), &[FILE]).is_err());
        assert!(parse_erlc(b"", None, &[FILE]).is_err());
        assert!(parse_erlc(b"compile failed\n", Some(1), &[FILE]).is_err());
        assert!(parse_erlc(&[0xff], Some(1), &[FILE]).is_err());
    }
}
