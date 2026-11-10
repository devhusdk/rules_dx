use super::gnu_diagnostics::{Dialect, Disposition, read};
use super::{FileFinding, ParseError};
use crate::ColumnEncoding;
use crate::ToolSeverity;

const DIALECT: Dialect = Dialect {
    tool: "elixirc",
    encoding: ColumnEncoding::Scalars,
    severity: |word| match word {
        "error" => Some(Disposition::Report(ToolSeverity::Error)),
        "warning" => Some(Disposition::Report(ToolSeverity::Warning)),
        _ => None,
    },
    bracket_rule: false,
};

/// The launcher prints this when the VM runs without a UTF-8 file name encoding. It has no location.
const LATIN1: &str = "native name encoding of latin1";

/// The checked source elixirc means by `path`, which may lack the leading directories.
fn checked_path<'a>(path: &'a str, files: &[&'a str]) -> &'a str {
    if files.contains(&path) {
        return path;
    }
    let mut matches = files
        .iter()
        .copied()
        .filter(|file| file.ends_with(&format!("/{path}")));
    match (matches.next(), matches.next()) {
        (Some(file), None) => file,
        _ => path,
    }
}

/// Splits `lib/a.ex:3:5: Mod.fun/1` into the path, the line and the column.
fn location(text: &str) -> Option<(&str, u64, u64)> {
    let mut parts = text.split(':');
    let path = parts.next()?;
    let line = parts.next()?.trim().parse().ok()?;
    let column = parts
        .next()
        .and_then(|part| part.trim().parse().ok())
        .unwrap_or(1);
    Some((path, line, column))
}

/// Turns each `warning:` or `error:` block into one `file:line:col: severity: message` line.
fn normalized(text: &str, files: &[&str]) -> Result<String, ParseError> {
    let mut out = String::with_capacity(text.len());
    let mut pending: Option<(&str, &str)> = None;
    for raw in text.lines() {
        let line = raw.trim();
        let header = ["warning", "error"]
            .into_iter()
            .find_map(|word| Some((word, line.strip_prefix(word)?.strip_prefix(": ")?)));
        if let Some((word, message)) = header {
            if let Some(("error", message)) = pending {
                return Err(unlocated(message));
            }
            pending = (!message.contains(LATIN1)).then_some((word, message));
        } else if let Some(at) = line.strip_prefix("└─ ") {
            let (Some((word, message)), Some((path, number, column))) = (pending, location(at))
            else {
                continue;
            };
            pending = None;
            out.push_str(&format!(
                "{}:{number}:{column}: {word}: {message}\n",
                checked_path(path, files)
            ));
        }
    }
    match pending {
        Some(("error", message)) => Err(unlocated(message)),
        _ => Ok(out),
    }
}

fn unlocated(message: &str) -> ParseError {
    ParseError::shape("elixirc", format!("error without a location: {message}"))
}

/// `elixirc` output. A nonzero exit that printed no diagnostic is an error.
pub fn parse_elixirc(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let Ok(text) = std::str::from_utf8(output) else {
        return read(&DIALECT, output, code, files);
    };
    read(&DIALECT, normalized(text, files)?.as_bytes(), code, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "lib/x.ex";

    const WARNINGS: &str = "    warning: variable \"unused\" is unused (if the variable is not meant to be used, prefix it with an underscore)
    │
  3 │     unused = 1
    │     ~~~~~~
    │
    └─ lib/x.ex:3:5: Dxx.run/1

    warning: function other/0 is unused
    │
  9 │   defp other do
    │        ~
    │
    └─ lib/x.ex:9:8: Dxx (module)

";

    #[test]
    fn elixirc_reports_unused_variables_and_functions() {
        let found = parse_elixirc(WARNINGS.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].file, FILE);
        assert_eq!(found[0].finding.severity, ToolSeverity::Warning);
        assert_eq!(found[0].finding.start.line, 3);
        assert_eq!(found[0].finding.start.column, 5);
        assert!(
            found[0]
                .finding
                .message
                .starts_with("variable \"unused\" is unused")
        );
        assert_eq!(found[1].finding.start.line, 9);
        assert_eq!(found[1].finding.message, "function other/0 is unused");
    }

    #[test]
    fn elixirc_reports_errors_and_ignores_the_trace() {
        let text = "== Compilation error in file lib/x.ex ==
** (SyntaxError) invalid syntax found on lib/x.ex:4:3:
    error: syntax error before: end
    │
  4 │   end
    │   ^
    │
    └─ lib/x.ex:4:3
    (elixir 1.20.4) lib/kernel/parallel_compiler.ex:548: anonymous fn/5 in Kernel.ParallelCompiler.spawn_workers/8
";
        let found = parse_elixirc(text.as_bytes(), Some(1), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.message, "syntax error before: end");
    }

    #[test]
    fn elixirc_skips_the_latin1_launcher_warning() {
        let text = format!(
            "warning: the VM is running with native name encoding of latin1 which may cause Elixir to malfunction\n\n{WARNINGS}"
        );
        let found = parse_elixirc(text.as_bytes(), Some(0), &[FILE]).expect("parsed");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn elixirc_maps_a_shortened_path_to_the_checked_source() {
        let found = parse_elixirc(
            WARNINGS.as_bytes(),
            Some(0),
            &["pkg/lib/x.ex", "pkg/lib/y.ex"],
        )
        .expect("parsed");
        assert_eq!(found[0].file, "pkg/lib/x.ex");
    }

    #[test]
    fn elixirc_drops_foreign_warnings_and_fails_on_foreign_errors() {
        assert!(
            parse_elixirc(WARNINGS.as_bytes(), Some(0), &["lib/y.ex"])
                .expect("parsed")
                .is_empty()
        );
        let error = "    error: undefined function foo/1\n    └─ lib/other.ex:3:5: E.f/1\n";
        assert!(parse_elixirc(error.as_bytes(), Some(1), &[FILE]).is_err());
    }

    #[test]
    fn elixirc_fails_closed() {
        assert!(
            parse_elixirc(b"", Some(0), &[FILE])
                .expect("clean")
                .is_empty()
        );
        assert!(parse_elixirc(b"", Some(1), &[FILE]).is_err());
        assert!(parse_elixirc(b"", None, &[FILE]).is_err());
        assert!(parse_elixirc(b"compile failed\n", Some(1), &[FILE]).is_err());
        assert!(parse_elixirc(b"    error: boom\n", Some(1), &[FILE]).is_err());
        assert!(parse_elixirc(&[0xff], Some(1), &[FILE]).is_err());
    }
}
