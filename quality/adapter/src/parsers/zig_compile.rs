use super::zig_ast_check::zig_errors;
use super::{as_text, code_name, prefixed, FileFinding, ParseError};

const TOOL: &str = "zig";

/// `zig build-exe|build-lib|test -fno-emit-bin` output: `path:line:col: error:` lines on stderr.
///
/// Fails closed: a nonzero exit without an error for an owned file, or an error in a file
/// that is not owned, is never a clean run.
pub fn parse_zig_compile(
    output: &[u8],
    code: Option<i32>,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let errors = zig_errors(TOOL, output, files, "compile")?;
    for line in as_text(TOOL, output)?.lines() {
        if line.contains(": error:") && prefixed(line, files).is_none() {
            let path = line.split(':').next().unwrap_or(line);
            return Err(ParseError::UnknownFile {
                tool: TOOL,
                path: path.to_owned(),
            });
        }
    }
    match code {
        Some(0) if errors.is_empty() => Ok(errors),
        Some(0) => Err(ParseError::shape(
            TOOL,
            "exit 0 with error diagnostics (want 1)",
        )),
        Some(_) if !errors.is_empty() => Ok(errors),
        other => {
            let first = as_text(TOOL, output)?
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("no output");
            Err(ParseError::shape(
                TOOL,
                format!(
                    "exit {} without a diagnostic for a checked file: {first}",
                    code_name(other)
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: [&str; 1] = ["zig/a.zig"];

    #[test]
    fn clean_run_has_no_findings() {
        assert!(parse_zig_compile(b"", Some(0), &FILES)
            .expect("parsed")
            .is_empty());
    }

    #[test]
    fn semantic_errors_are_reported_and_notes_skipped() {
        let err = "zig/a.zig:4:21: error: expected type 'i32', found '*const [5:0]u8'\n    const x: i32 = \"hello\";\n                    ^~~~~~~\nzig/a.zig:3:5: note: declared here\nreferenced by:\n    main: zig/a.zig:9:1\n";
        let found = parse_zig_compile(err.as_bytes(), Some(1), &FILES).expect("parsed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "zig/a.zig");
        assert_eq!(found[0].finding.rule_id, "compile");
        assert_eq!(found[0].finding.start.line, 4);
        assert_eq!(found[0].finding.start.column, 21);
    }

    #[test]
    fn failures_are_errors() {
        assert!(parse_zig_compile(b"", Some(1), &FILES).is_err());
        assert!(parse_zig_compile(b"error: unable to find zig lib\n", Some(1), &FILES).is_err());
        assert!(parse_zig_compile(b"zig/a.zig:1:1: error: bad\n", Some(0), &FILES).is_err());
        assert!(parse_zig_compile(b"", None, &FILES).is_err());
    }

    #[test]
    fn an_error_in_a_foreign_file_is_rejected() {
        let err = "zig/gen.zig:2:3: error: bad\nzig/a.zig:1:1: error: also bad\n";
        let error = parse_zig_compile(err.as_bytes(), Some(1), &FILES).expect_err("foreign");
        assert!(error.to_string().contains("zig/gen.zig"), "{error}");
    }
}
